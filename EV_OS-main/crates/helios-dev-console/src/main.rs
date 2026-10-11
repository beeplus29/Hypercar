use std::{
    env,
    error::Error,
    io,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::{Path, PathBuf},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use axum::{
    Json, Router,
    body::Body,
    extract::{DefaultBodyLimit, Multipart, Path as AxumPath, State},
    http::{HeaderValue, Request, StatusCode, header},
    middleware::{self, Next},
    response::{Html, IntoResponse, Response},
    routing::get,
};
use serde::{Deserialize, Serialize};
use tokio::{process::Command, sync::Mutex, time::Duration};

const ADDRESS: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 4318);
const PAGE: &str = include_str!("../web/index.html");
const MAX_DOCUMENT_SIZE: usize = 15 * 1024 * 1024;
const MAX_DOCUMENTS_PER_PART: usize = 50;

#[derive(Clone)]
struct AppState {
    data_path: PathBuf,
    records: Arc<Mutex<Vec<HardwareRecord>>>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct HardwareRecord {
    id: u64,
    name: String,
    category: String,
    manufacturer: String,
    model: String,
    serial_number: Option<String>,
    connection: Option<ConnectionProfile>,
    #[serde(default)]
    documents: Vec<HardwareDocument>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct HardwareDocument {
    id: u64,
    filename: String,
    format: DocumentFormat,
    extraction_message: String,
    specifications: Vec<ExtractedSpecification>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum DocumentFormat {
    Pdf,
    Text,
    Csv,
    Png,
    Jpeg,
}

impl DocumentFormat {
    fn extension(&self) -> &'static str {
        match self {
            Self::Pdf => "pdf",
            Self::Text => "txt",
            Self::Csv => "csv",
            Self::Png => "png",
            Self::Jpeg => "jpg",
        }
    }

    fn content_type(&self) -> &'static str {
        match self {
            Self::Pdf => "application/pdf",
            Self::Text => "text/plain; charset=utf-8",
            Self::Csv => "text/csv; charset=utf-8",
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ExtractedSpecification {
    id: u64,
    name: String,
    value: String,
    unit: String,
    source_excerpt: String,
    verified: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct HardwareInput {
    name: String,
    category: String,
    manufacturer: String,
    model: String,
    serial_number: Option<String>,
    connection: Option<ConnectionProfile>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ConnectionProfile {
    transport: Transport,
    interface_name: Option<String>,
    endpoint: Option<String>,
    bitrate: Option<u32>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum Transport {
    SocketCan,
    VhalGrpc,
    Dds,
}

#[derive(Serialize)]
struct InterfaceReport {
    supported: bool,
    message: String,
    interfaces: Vec<CanInterface>,
}

#[derive(Serialize)]
struct CanInterface {
    name: String,
    state: String,
}

#[derive(Serialize)]
struct ErrorBody {
    error: String,
}

#[derive(Deserialize)]
struct VerifySpecificationInput {
    verified: bool,
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn bad_request(message: impl Into<String>) -> Self {
        Self { status: StatusCode::BAD_REQUEST, message: message.into() }
    }

    fn internal(error: impl std::fmt::Display) -> Self {
        Self { status: StatusCode::INTERNAL_SERVER_ERROR, message: error.to_string() }
    }

    fn unprocessable(message: impl Into<String>) -> Self {
        Self { status: StatusCode::UNPROCESSABLE_ENTITY, message: message.into() }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(ErrorBody { error: self.message })).into_response()
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let data_path = data_path()?;
    let records = load_records(&data_path).await?;
    println!("Inventory file: {}", data_path.display());
    let state = AppState { data_path, records: Arc::new(Mutex::new(records)) };
    let app = Router::new()
        .route("/", get(index))
        .route("/api/hardware", get(list_hardware).post(add_hardware))
        .route("/api/hardware/{id}", axum::routing::delete(delete_hardware).put(update_hardware))
        .route("/api/hardware/{hardware_id}/documents", axum::routing::post(upload_document))
        .route("/api/hardware/{hardware_id}/documents/{document_id}/file", get(download_document))
        .route("/api/hardware/{hardware_id}/documents/{document_id}", axum::routing::delete(delete_document))
        .route(
            "/api/hardware/{hardware_id}/documents/{document_id}/specifications/{specification_id}",
            axum::routing::put(verify_specification),
        )
        .route("/api/interfaces", get(list_interfaces))
        .with_state(state)
        .layer(DefaultBodyLimit::max(MAX_DOCUMENT_SIZE + 1024 * 1024))
        .layer(middleware::from_fn(require_local_host));
    let listener = tokio::net::TcpListener::bind(ADDRESS).await?;
    println!("HELIOS developer hardware console: http://{ADDRESS}");
    println!("HELIOS developer hardware console: http://{ADDRESS}");

    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            if let Err(error) = tokio::signal::ctrl_c().await {
                eprintln!("Failed to listen for shutdown signal: {error}");
            }
        })
        .await?;

    Ok(())
}

fn data_path() -> Result<PathBuf, Box<dyn Error>> {
    if let Some(path) = env::var_os("HELIOS_DEV_CONSOLE_DATA") {
        if path.is_empty() {
            return Err("HELIOS_DEV_CONSOLE_DATA must not be empty".into());
        }
        return Ok(PathBuf::from(path));
    }

    let home = env::var_os("HOME").ok_or("HOME is not set; set HELIOS_DEV_CONSOLE_DATA")?;
    Ok(PathBuf::from(home).join(".local").join("share").join("helios-dev-console").join("inventory.json"))
}

async fn load_records(path: &Path) -> Result<Vec<HardwareRecord>, Box<dyn Error>> {
    match tokio::fs::read(path).await {
        Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(error.into()),
    }
}

async fn persist_records(path: &Path, records: &[HardwareRecord]) -> Result<(), ApiError> {
    let parent =
        path.parent().filter(|parent| !parent.as_os_str().is_empty()).unwrap_or_else(|| Path::new("."));
    tokio::fs::create_dir_all(parent).await.map_err(ApiError::internal)?;

    let temporary_path = path.with_extension("tmp");
    let serialized = serde_json::to_vec_pretty(records).map_err(ApiError::internal)?;
    tokio::fs::write(&temporary_path, serialized).await.map_err(ApiError::internal)?;
    if let Err(error) = tokio::fs::rename(&temporary_path, path).await {
        let _ = tokio::fs::remove_file(&temporary_path).await;
        return Err(ApiError::internal(error));
    }
    Ok(())
}

async fn index() -> Html<&'static str> {
    Html(PAGE)
}

async fn list_hardware(State(state): State<AppState>) -> Json<Vec<HardwareRecord>> {
    Json(state.records.lock().await.clone())
}

async fn add_hardware(
    State(state): State<AppState>,
    Json(input): Json<HardwareInput>,
) -> Result<(StatusCode, Json<HardwareRecord>), ApiError> {
    validate_input(&input)?;
    let mut records = state.records.lock().await;
    let id = records
        .iter()
        .map(|record| record.id)
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| ApiError::internal("Hardware record ID space is exhausted"))?;
    let record = build_record(id, input);

    let mut updated_records = records.clone();
    updated_records.push(record.clone());
    persist_records(&state.data_path, &updated_records).await?;
    *records = updated_records;
    Ok((StatusCode::CREATED, Json(record)))
}

async fn update_hardware(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<u64>,
    Json(input): Json<HardwareInput>,
) -> Result<Json<HardwareRecord>, ApiError> {
    validate_input(&input)?;
    let mut records = state.records.lock().await;
    let Some(index) = records.iter().position(|record| record.id == id) else {
        return Err(ApiError {
            status: StatusCode::NOT_FOUND,
            message: format!("Hardware record {id} was not found"),
        });
    };
    let mut record = build_record(id, input);
    record.documents = records[index].documents.clone();
    let mut updated_records = records.clone();
    updated_records[index] = record.clone();
    persist_records(&state.data_path, &updated_records).await?;
    *records = updated_records;
    Ok(Json(record))
}

async fn upload_document(
    State(state): State<AppState>,
    AxumPath(hardware_id): AxumPath<u64>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<HardwareDocument>), ApiError> {
    let mut upload = None;
    while let Some(field) =
        multipart.next_field().await.map_err(|error| ApiError::bad_request(error.to_string()))?
    {
        if field.name() != Some("file") {
            return Err(ApiError::bad_request("Upload must contain a single file field"));
        }
        if upload.is_some() {
            return Err(ApiError::bad_request("Upload one document at a time"));
        }
        let filename = field
            .file_name()
            .map(str::to_owned)
            .ok_or_else(|| ApiError::bad_request("Uploaded document must have a filename"))?;
        let bytes = field.bytes().await.map_err(|error| ApiError::bad_request(error.to_string()))?;
        if bytes.is_empty() {
            return Err(ApiError::bad_request("The uploaded document is empty"));
        }
        if bytes.len() > MAX_DOCUMENT_SIZE {
            return Err(ApiError::bad_request("Documents must be 15 MB or smaller"));
        }
        upload = Some((filename, bytes));
    }

    let (filename, bytes) = upload.ok_or_else(|| ApiError::bad_request("Choose a document to upload"))?;
    let format = document_format(&filename, &bytes)?;
    let filename = display_filename(&filename);
    let mut records = state.records.lock().await;
    let Some(index) = records.iter().position(|record| record.id == hardware_id) else {
        return Err(ApiError {
            status: StatusCode::NOT_FOUND,
            message: format!("Hardware record {hardware_id} was not found"),
        });
    };
    if records[index].documents.len() >= MAX_DOCUMENTS_PER_PART {
        return Err(ApiError::bad_request(format!(
            "Each part can have up to {MAX_DOCUMENTS_PER_PART} documents"
        )));
    }
    let document_id = records[index]
        .documents
        .iter()
        .map(|document| document.id)
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| ApiError::internal("Document ID space is exhausted"))?;
    let directory = document_directory(&state.data_path);
    tokio::fs::create_dir_all(&directory).await.map_err(ApiError::internal)?;
    let final_path = stored_document_path(&state.data_path, hardware_id, document_id, &format);
    let unique = SystemTime::now().duration_since(UNIX_EPOCH).map_err(ApiError::internal)?.as_nanos();
    let temporary_path =
        directory.join(format!("upload-{hardware_id}-{document_id}-{unique}.{}", format.extension()));
    tokio::fs::write(&temporary_path, &bytes).await.map_err(ApiError::internal)?;

    let extraction = extract_document_text(&format, &bytes, &temporary_path).await;
    let text = match extraction {
        Ok(text) => text,
        Err(error) => {
            let _ = tokio::fs::remove_file(&temporary_path).await;
            return Err(error);
        }
    };
    if text.len() > 10 * 1024 * 1024 {
        let _ = tokio::fs::remove_file(&temporary_path).await;
        return Err(ApiError::unprocessable(
            "Extracted text is larger than 10 MB. Try a shorter or simpler document.",
        ));
    }
    let specifications = extract_specifications(&text)?;
    let extraction_message = if text.trim().is_empty() {
        "No selectable text was found. If this is a scanned PDF, upload page images; image OCR requires Tesseract.".to_owned()
    } else if specifications.is_empty() {
        "Text was read, but no supported unit-labelled specs were recognized. The document is saved for you to review.".to_owned()
    } else {
        format!(
            "Found {} possible value(s). Review each source excerpt before adding it to this part.",
            specifications.len()
        )
    };
    let document = HardwareDocument { id: document_id, filename, format, extraction_message, specifications };

    tokio::fs::rename(&temporary_path, &final_path).await.map_err(|error| {
        let _ = std::fs::remove_file(&temporary_path);
        ApiError::internal(error)
    })?;
    let mut updated_records = records.clone();
    updated_records[index].documents.push(document.clone());
    if let Err(error) = persist_records(&state.data_path, &updated_records).await {
        let _ = tokio::fs::remove_file(&final_path).await;
        return Err(error);
    }
    *records = updated_records;
    Ok((StatusCode::CREATED, Json(document)))
}

async fn download_document(
    State(state): State<AppState>,
    AxumPath((hardware_id, document_id)): AxumPath<(u64, u64)>,
) -> Result<Response, ApiError> {
    let records = state.records.lock().await;
    let document = records
        .iter()
        .find(|record| record.id == hardware_id)
        .and_then(|record| record.documents.iter().find(|document| document.id == document_id))
        .ok_or_else(|| ApiError {
            status: StatusCode::NOT_FOUND,
            message: "Document was not found".to_owned(),
        })?;
    let bytes =
        tokio::fs::read(stored_document_path(&state.data_path, hardware_id, document_id, &document.format))
            .await
            .map_err(ApiError::internal)?;
    let mut response = Response::new(Body::from(bytes));
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, HeaderValue::from_static(document.format.content_type()));
    response.headers_mut().insert(header::CONTENT_DISPOSITION, HeaderValue::from_static("attachment"));
    Ok(response)
}

async fn delete_document(
    State(state): State<AppState>,
    AxumPath((hardware_id, document_id)): AxumPath<(u64, u64)>,
) -> Result<StatusCode, ApiError> {
    let mut records = state.records.lock().await;
    let Some(index) = records.iter().position(|record| record.id == hardware_id) else {
        return Err(ApiError {
            status: StatusCode::NOT_FOUND,
            message: format!("Hardware record {hardware_id} was not found"),
        });
    };
    let Some(document) = records[index].documents.iter().find(|document| document.id == document_id) else {
        return Err(ApiError {
            status: StatusCode::NOT_FOUND,
            message: format!("Document {document_id} was not found"),
        });
    };
    let format = document.format.clone();
    let mut updated_records = records.clone();
    updated_records[index].documents.retain(|document| document.id != document_id);
    persist_records(&state.data_path, &updated_records).await?;
    *records = updated_records;
    match tokio::fs::remove_file(stored_document_path(&state.data_path, hardware_id, document_id, &format))
        .await
    {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(ApiError::internal(error)),
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn verify_specification(
    State(state): State<AppState>,
    AxumPath((hardware_id, document_id, specification_id)): AxumPath<(u64, u64, u64)>,
    Json(input): Json<VerifySpecificationInput>,
) -> Result<Json<ExtractedSpecification>, ApiError> {
    let mut records = state.records.lock().await;
    let Some(record_index) = records.iter().position(|record| record.id == hardware_id) else {
        return Err(ApiError {
            status: StatusCode::NOT_FOUND,
            message: format!("Hardware record {hardware_id} was not found"),
        });
    };
    let mut updated_records = records.clone();
    let Some(document) =
        updated_records[record_index].documents.iter_mut().find(|document| document.id == document_id)
    else {
        return Err(ApiError {
            status: StatusCode::NOT_FOUND,
            message: format!("Document {document_id} was not found"),
        });
    };
    let Some(specification) =
        document.specifications.iter_mut().find(|specification| specification.id == specification_id)
    else {
        return Err(ApiError {
            status: StatusCode::NOT_FOUND,
            message: format!("Suggested value {specification_id} was not found"),
        });
    };
    specification.verified = input.verified;
    let result = specification.clone();
    persist_records(&state.data_path, &updated_records).await?;
    *records = updated_records;
    Ok(Json(result))
}

fn document_directory(inventory_path: &Path) -> PathBuf {
    inventory_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .join("documents")
}

fn stored_document_path(
    inventory_path: &Path,
    hardware_id: u64,
    document_id: u64,
    format: &DocumentFormat,
) -> PathBuf {
    document_directory(inventory_path).join(format!("{hardware_id}-{document_id}.{}", format.extension()))
}

fn document_format(filename: &str, bytes: &[u8]) -> Result<DocumentFormat, ApiError> {
    let extension = Path::new(filename)
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match extension.as_str() {
        "pdf" if bytes.starts_with(b"%PDF-") => Ok(DocumentFormat::Pdf),
        "pdf" => Err(ApiError::bad_request("The file does not appear to be a valid PDF")),
        "txt" | "text" | "csv" => {
            if bytes.contains(&0) || std::str::from_utf8(bytes).is_err() {
                return Err(ApiError::bad_request("Text and CSV documents must contain readable UTF-8 text"));
            }
            if extension == "csv" { Ok(DocumentFormat::Csv) } else { Ok(DocumentFormat::Text) }
        }
        "png" if bytes.starts_with(b"\x89PNG\r\n\x1a\n") => Ok(DocumentFormat::Png),
        "jpg" | "jpeg" if bytes.starts_with(&[0xff, 0xd8, 0xff]) => Ok(DocumentFormat::Jpeg),
        "png" | "jpg" | "jpeg" => {
            Err(ApiError::bad_request("The image contents do not match its PNG or JPEG filename"))
        }
        _ => Err(ApiError::bad_request("Supported documents are PDF, TXT, CSV, PNG, and JPEG")),
    }
}

fn display_filename(filename: &str) -> String {
    let base_name = filename.rsplit(['/', '\\']).next().unwrap_or("document");
    let cleaned: String = base_name.chars().filter(|character| !character.is_control()).take(180).collect();
    if cleaned.trim().is_empty() { "document".to_owned() } else { cleaned }
}

async fn extract_document_text(
    format: &DocumentFormat,
    bytes: &[u8],
    temporary_path: &Path,
) -> Result<String, ApiError> {
    match format {
        DocumentFormat::Pdf => {
            let bytes = bytes.to_vec();
            tokio::task::spawn_blocking(move || pdf_extract::extract_text_from_mem(&bytes))
                .await
                .map_err(ApiError::internal)?
                .map_err(|error| ApiError::unprocessable(error.to_string()))
        }
        DocumentFormat::Text | DocumentFormat::Csv => {
            String::from_utf8(bytes.to_vec()).map_err(|error| ApiError::unprocessable(error.to_string()))
        }
        DocumentFormat::Png | DocumentFormat::Jpeg => {
            let mut command = Command::new("tesseract");
            command.kill_on_drop(true);
            command.arg(temporary_path).arg("stdout").arg("--psm").arg("6");
            let output = tokio::time::timeout(
                Duration::from_secs(45),
                command.output(),
            )
            .await
            .map_err(|_| ApiError::unprocessable("Image text recognition took too long; try a smaller image"))?
            .map_err(|error| {
                if error.kind() == io::ErrorKind::NotFound {
                    ApiError::unprocessable(
                        "Image OCR is not installed. Install Tesseract OCR and ensure `tesseract` is on PATH, then restart the console.",
                    )
                } else {
                    ApiError::internal(error)
                }
            })?;
            if !output.status.success() {
                let detail = String::from_utf8_lossy(&output.stderr);
                return Err(ApiError::unprocessable(format!(
                    "Image OCR failed: {}",
                    detail.lines().next().unwrap_or("Tesseract could not read the image")
                )));
            }
            String::from_utf8(output.stdout).map_err(|error| ApiError::unprocessable(error.to_string()))
        }
    }
}

fn extract_specifications(text: &str) -> Result<Vec<ExtractedSpecification>, ApiError> {
    let pattern = regex::Regex::new(
        r"(?i)([-+]?\d+(?:[.,]\d+)?)\s*[,;:]?\s*(kwh|mwh|wh|mah|ah|kw|mw|w|kv|v|ka|ma|a|n[·.]?m|rpm|°\s?c|degc|celsius|kg|g|lb)\b",
    )
    .map_err(ApiError::internal)?;
    let mut specifications = Vec::new();
    for line in text.lines().take(20_000) {
        let line = line.trim();
        if line.is_empty() || line.len() > 600 {
            continue;
        }
        let lower = line.to_ascii_lowercase();
        let Some(name) = specification_name(&lower) else {
            continue;
        };
        for captures in pattern.captures_iter(line) {
            let raw_unit = captures[2].replace(' ', "");
            if !unit_matches(name, &raw_unit) {
                continue;
            }
            let raw_value = &captures[1];
            let value = if let Some((whole, fraction)) = raw_value.split_once(',')
                && fraction.len() <= 2
                && !raw_value.contains('.')
            {
                format!("{whole}.{fraction}")
            } else if raw_value.contains(',') {
                continue;
            } else {
                raw_value.to_owned()
            };
            if value.parse::<f64>().map_or(true, |number| !number.is_finite()) {
                continue;
            }
            if specifications.iter().any(|existing: &ExtractedSpecification| {
                existing.name == name && existing.value == value && existing.unit == raw_unit
            }) {
                continue;
            }
            let source_excerpt: String = line.chars().take(240).collect();
            specifications.push(ExtractedSpecification {
                id: specifications.len() as u64 + 1,
                name: name.to_owned(),
                value,
                unit: display_unit(&raw_unit),
                source_excerpt,
                verified: false,
            });
            if specifications.len() >= 100 {
                return Ok(specifications);
            }
        }
    }
    Ok(specifications)
}

fn specification_name(line: &str) -> Option<&'static str> {
    if line.contains("voltage") || line.contains("volt") {
        Some("Voltage")
    } else if line.contains("current") || line.contains("amperage") {
        Some("Current")
    } else if line.contains("power") || line.contains("wattage") {
        Some("Power")
    } else if line.contains("torque") {
        Some("Torque")
    } else if line.contains("speed") || line.contains("rpm") {
        Some("Speed")
    } else if line.contains("capacity") || line.contains("energy") {
        Some("Capacity")
    } else if line.contains("weight") || line.contains("mass") {
        Some("Mass")
    } else if line.contains("temperature") || line.contains("temp.") || line.contains("thermal") {
        Some("Temperature")
    } else {
        None
    }
}

fn unit_matches(name: &str, unit: &str) -> bool {
    let unit = unit.to_ascii_lowercase();
    match name {
        "Voltage" => matches!(unit.as_str(), "v" | "kv"),
        "Current" => matches!(unit.as_str(), "a" | "ma" | "ka"),
        "Power" => matches!(unit.as_str(), "w" | "kw" | "mw"),
        "Torque" => matches!(unit.as_str(), "nm" | "n.m" | "n·m"),
        "Speed" => unit == "rpm",
        "Capacity" => matches!(unit.as_str(), "wh" | "kwh" | "mwh" | "ah" | "mah"),
        "Mass" => matches!(unit.as_str(), "g" | "kg" | "lb"),
        "Temperature" => matches!(unit.as_str(), "c" | "°c" | "degc" | "celsius"),
        _ => false,
    }
}

fn display_unit(unit: &str) -> String {
    match unit.to_ascii_lowercase().as_str() {
        "v" => "V",
        "kv" => "kV",
        "a" => "A",
        "ma" => "mA",
        "ka" => "kA",
        "w" => "W",
        "kw" => "kW",
        "mw" => "MW",
        "nm" | "n.m" => "Nm",
        "n·m" => "N·m",
        "rpm" => "rpm",
        "wh" => "Wh",
        "kwh" => "kWh",
        "mwh" => "MWh",
        "ah" => "Ah",
        "mah" => "mAh",
        "g" => "g",
        "kg" => "kg",
        "lb" => "lb",
        "c" | "°c" | "degc" | "celsius" => "°C",
        _ => unit,
    }
    .to_owned()
}

fn build_record(id: u64, input: HardwareInput) -> HardwareRecord {
    HardwareRecord {
        id,
        name: input.name.trim().to_owned(),
        category: input.category.trim().to_owned(),
        manufacturer: recorded_value(input.manufacturer),
        model: recorded_value(input.model),
        serial_number: input
            .serial_number
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty()),
        connection: input.connection,
        documents: Vec::new(),
    }
}

async fn delete_hardware(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<u64>,
) -> Result<StatusCode, ApiError> {
    let mut records = state.records.lock().await;
    let mut updated_records = records.clone();
    let old_len = updated_records.len();
    updated_records.retain(|record| record.id != id);
    if updated_records.len() == old_len {
        return Err(ApiError {
            status: StatusCode::NOT_FOUND,
            message: format!("Hardware record {id} was not found"),
        });
    }
    let removed_record = records
        .iter()
        .find(|record| record.id == id)
        .ok_or_else(|| ApiError::internal("Hardware record disappeared during deletion"))?
        .clone();
    persist_records(&state.data_path, &updated_records).await?;
    *records = updated_records;
    for document in removed_record.documents {
        match tokio::fs::remove_file(stored_document_path(
            &state.data_path,
            id,
            document.id,
            &document.format,
        ))
        .await
        {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(ApiError::internal(error)),
        }
    }
    Ok(StatusCode::NO_CONTENT)
}

fn recorded_value(value: String) -> String {
    let value = value.trim();
    if value.is_empty() { "Not recorded".to_owned() } else { value.to_owned() }
}

fn validate_input(input: &HardwareInput) -> Result<(), ApiError> {
    validate_required("Name", &input.name, 100)?;
    validate_required("Category", &input.category, 60)?;
    validate_optional("Manufacturer", &input.manufacturer, 100)?;
    validate_optional("Model", &input.model, 100)?;
    if let Some(serial_number) = &input.serial_number {
        validate_optional("Serial number", serial_number, 120)?;
    }

    if let Some(connection) = &input.connection {
        if let Some(interface_name) = &connection.interface_name {
            validate_optional("Interface name", interface_name, 15)?;
            if !interface_name.is_empty()
                && !interface_name.bytes().all(|byte| byte.is_ascii_alphanumeric() || b"_.-".contains(&byte))
            {
                return Err(ApiError::bad_request(
                    "Interface name may contain only letters, numbers, underscores, dots, and hyphens",
                ));
            }
        }
        if let Some(endpoint) = &connection.endpoint {
            validate_optional("Endpoint", endpoint, 255)?;
            if endpoint.chars().any(char::is_whitespace) {
                return Err(ApiError::bad_request("Endpoint must not contain whitespace"));
            }
        }
        if let Some(bitrate) = connection.bitrate
            && !(10_000..=10_000_000).contains(&bitrate)
        {
            return Err(ApiError::bad_request("Bitrate must be between 10000 and 10000000 bits per second"));
        }
        match &connection.transport {
            Transport::SocketCan if connection.interface_name.as_deref().unwrap_or("").is_empty() => {
                return Err(ApiError::bad_request("SocketCAN profiles require an interface name"));
            }
            Transport::VhalGrpc if connection.endpoint.as_deref().unwrap_or("").is_empty() => {
                return Err(ApiError::bad_request("VHAL gRPC profiles require an endpoint"));
            }
            _ => {}
        }
    }
    Ok(())
}

fn validate_required(label: &str, value: &str, max_len: usize) -> Result<(), ApiError> {
    if value.trim().is_empty() {
        return Err(ApiError::bad_request(format!("{label} is required")));
    }
    validate_optional(label, value, max_len)
}

fn validate_optional(label: &str, value: &str, max_len: usize) -> Result<(), ApiError> {
    if value.len() > max_len || value.chars().any(char::is_control) {
        return Err(ApiError::bad_request(format!(
            "{label} must be at most {max_len} bytes and contain no control characters"
        )));
    }
    Ok(())
}

async fn list_interfaces() -> Result<Json<InterfaceReport>, ApiError> {
    #[cfg(target_os = "linux")]
    {
        let root = Path::new("/sys/class/net");
        let mut directory = match tokio::fs::read_dir(root).await {
            Ok(directory) => directory,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(Json(InterfaceReport {
                    supported: false,
                    message: "SocketCAN discovery requires Linux network sysfs.".to_owned(),
                    interfaces: Vec::new(),
                }));
            }
            Err(error) => return Err(ApiError::internal(error)),
        };
        let mut interfaces = Vec::new();

        while let Some(entry) = directory.next_entry().await.map_err(ApiError::internal)? {
            let name = entry.file_name().to_string_lossy().into_owned();
            let interface_path = entry.path();
            let kind = match tokio::fs::read_to_string(interface_path.join("type")).await {
                Ok(kind) => kind,
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                Err(error) => return Err(ApiError::internal(error)),
            };
            if kind.trim() != "280" {
                continue;
            }
            let state = match tokio::fs::read_to_string(interface_path.join("operstate")).await {
                Ok(state) => state.trim().to_owned(),
                Err(error) if error.kind() == io::ErrorKind::NotFound => "unknown".to_owned(),
                Err(error) => return Err(ApiError::internal(error)),
            };
            interfaces.push(CanInterface { name, state });
        }

        let message = if interfaces.is_empty() {
            "No SocketCAN interfaces were found on this host.".to_owned()
        } else {
            "SocketCAN interface state is read-only; profiles are not applied to devices.".to_owned()
        };
        return Ok(Json(InterfaceReport { supported: true, message, interfaces }));
    }

    #[cfg(not(target_os = "linux"))]
    {
        Ok(Json(InterfaceReport {
            supported: false,
            message: "Live CAN discovery is supported on Linux hosts with SocketCAN.".to_owned(),
            interfaces: Vec::new(),
        }))
    }
}

async fn require_local_host(request: Request<Body>, next: Next) -> Response {
    let valid_host = request
        .headers()
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|host| matches!(host, "localhost:4318" | "127.0.0.1:4318"));
    let valid_origin = request.headers().get(header::ORIGIN).is_none_or(|origin| {
        origin == HeaderValue::from_static("http://localhost:4318")
            || origin == HeaderValue::from_static("http://127.0.0.1:4318")
    });

    if !valid_host || !valid_origin {
        return (
            StatusCode::FORBIDDEN,
            Json(ErrorBody {
                error: "This developer console accepts requests only from its local web UI.".to_owned(),
            }),
        )
            .into_response();
    }
    next.run(request).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_input() -> HardwareInput {
        HardwareInput {
            name: "Front inverter".to_owned(),
            category: "Powertrain".to_owned(),
            manufacturer: "Example".to_owned(),
            model: "INV-1".to_owned(),
            serial_number: None,
            connection: Some(ConnectionProfile {
                transport: Transport::SocketCan,
                interface_name: Some("can0".to_owned()),
                endpoint: None,
                bitrate: Some(500_000),
            }),
        }
    }

    #[test]
    fn accepts_valid_inventory_and_socketcan_profile() {
        assert!(validate_input(&sample_input()).is_ok());
    }

    #[test]
    fn accepts_inventory_when_manufacturer_and_model_are_unknown() {
        let mut input = sample_input();
        input.manufacturer.clear();
        input.model.clear();
        assert!(validate_input(&input).is_ok());
        assert_eq!(recorded_value(input.manufacturer), "Not recorded");
        assert_eq!(recorded_value(input.model), "Not recorded");
    }

    #[test]
    fn rejects_socketcan_profile_without_interface() {
        let mut input = sample_input();
        input.connection.as_mut().expect("sample profile").interface_name = None;
        assert!(validate_input(&input).is_err());
    }

    #[test]
    fn rejects_out_of_range_bitrate() {
        let mut input = sample_input();
        input.connection.as_mut().expect("sample profile").bitrate = Some(1);
        assert!(validate_input(&input).is_err());
    }

    #[test]
    fn rejects_interface_names_that_could_be_paths() {
        let mut input = sample_input();
        input.connection.as_mut().expect("sample profile").interface_name = Some("../can0".to_owned());
        assert!(validate_input(&input).is_err());
    }

    #[test]
    fn extracts_unit_labelled_suggestions_with_source_lines() {
        let specifications = extract_specifications(
            "Nominal voltage,800,V\nPeak power: 1.8 MW\nRated torque: 548 N·m\nMaximum speed: 17750 rpm\nUnrelated text: 24 V",
        )
        .expect("the extraction pattern is valid");
        assert_eq!(specifications.len(), 4);
        assert_eq!(specifications[0].name, "Voltage");
        assert_eq!(specifications[0].value, "800");
        assert_eq!(specifications[0].unit, "V");
        assert_eq!(specifications[0].source_excerpt, "Nominal voltage,800,V");
        assert!(!specifications[0].verified);
        assert_eq!(specifications[1].name, "Power");
        assert_eq!(specifications[1].unit, "MW");
        assert_eq!(specifications[2].name, "Torque");
        assert_eq!(specifications[3].name, "Speed");
    }

    #[test]
    fn accepts_only_supported_document_signatures_and_text() {
        assert!(matches!(document_format("manual.pdf", b"%PDF-1.7"), Ok(DocumentFormat::Pdf)));
        assert!(matches!(document_format("spec.csv", b"Voltage,800,V"), Ok(DocumentFormat::Csv)));
        assert!(document_format("manual.pdf", b"not a pdf").is_err());
        assert!(document_format("photo.png", b"not a png").is_err());
        assert!(document_format("unknown.exe", b"content").is_err());
    }

    #[test]
    fn strips_paths_and_control_characters_from_display_names() {
        assert_eq!(display_filename("../../specs\r\n.pdf"), "specs.pdf");
    }

    #[test]
    fn loads_inventory_records_written_before_document_support() {
        let old_record = r#"{"id":1,"name":"Motor","category":"Powertrain","manufacturer":"Example","model":"M1","serial_number":null,"connection":null}"#;
        let record: HardwareRecord =
            serde_json::from_str(old_record).expect("old inventory remains readable");
        assert!(record.documents.is_empty());
    }
}
