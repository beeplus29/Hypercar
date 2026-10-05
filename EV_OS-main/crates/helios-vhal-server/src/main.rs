use tonic::{transport::Server, Request, Response, Status};
use tokio_stream::wrappers::ReceiverStream;

pub mod vehicle {
    tonic::include_proto!("android.hardware.automotive.vehicle.proto");
}

use vehicle::vehicle_server_server::{VehicleServer, VehicleServerServer};
use vehicle::{
    DumpOptions, DumpResult, GetMinMaxSupportedValuesRequest, GetMinMaxSupportedValuesResult,
    GetSupportedValuesListsRequest, GetSupportedValuesListsResult, GetValueResults,
    SetValueResults, SubscribeRequest, SupportedValuesChange, UnsubscribeRequest,
    UpdateSampleRateRequest, VehicleHalCallStatus, VehiclePropConfig, VehiclePropValueRequests,
    VehiclePropValues, VehicleAreaConfig
};

// Android VHAL Property IDs (from AOSP 17)
const PERF_VEHICLE_SPEED: i32 = 0x11600207;
const EV_BATTERY_LEVEL: i32 = 0x11600309;
const TIRE_PRESSURE: i32 = 0x17600309;
const CURRENT_GEAR: i32 = 0x11400401;
const PERF_STEERING_ANGLE: i32 = 0x11600209;
const EV_BATTERY_AVERAGE_TEMPERATURE: i32 = 0x1160030E;

// Access Types
const ACCESS_READ: i32 = 1;
// const ACCESS_WRITE: i32 = 2;
// const ACCESS_READ_WRITE: i32 = 3;

// Change Modes
const CHANGE_MODE_CONTINUOUS: i32 = 2;
const CHANGE_MODE_ON_CHANGE: i32 = 1;

#[derive(Default)]
pub struct HeliosVhalService {}

impl HeliosVhalService {
    fn create_prop_config(prop_id: i32, access: i32, change_mode: i32, min_sample: f32, max_sample: f32) -> VehiclePropConfig {
        VehiclePropConfig {
            prop: prop_id,
            access,
            change_mode,
            area_configs: vec![],
            config_array: vec![],
            config_string: String::new(),
            min_sample_rate: min_sample,
            max_sample_rate: max_sample,
        }
    }
}

#[tonic::async_trait]
impl VehicleServer for HeliosVhalService {
    type GetAllPropertyConfigStream = ReceiverStream<Result<VehiclePropConfig, Status>>;
    type StartPropertyValuesStreamStream = ReceiverStream<Result<VehiclePropValues, Status>>;
    type StartSupportedValuesChangeStreamStream = ReceiverStream<Result<SupportedValuesChange, Status>>;

    async fn get_all_property_config(
        &self,
        _request: Request<()>,
    ) -> Result<Response<Self::GetAllPropertyConfigStream>, Status> {
        let (tx, rx) = tokio::sync::mpsc::channel(16);
        
        tokio::spawn(async move {
            // 1. Vehicle Speed (Continuous, 1Hz - 100Hz)
            let speed_config = Self::create_prop_config(PERF_VEHICLE_SPEED, ACCESS_READ, CHANGE_MODE_CONTINUOUS, 1.0, 100.0);
            tx.send(Ok(speed_config)).await.unwrap();

            // 2. Battery Level (Continuous, 1Hz - 10Hz)
            let batt_config = Self::create_prop_config(EV_BATTERY_LEVEL, ACCESS_READ, CHANGE_MODE_CONTINUOUS, 1.0, 10.0);
            tx.send(Ok(batt_config)).await.unwrap();

            // 3. Tire Pressure (Continuous, 1Hz - 50Hz, requires Area configs for 4 wheels)
            let mut tp_config = Self::create_prop_config(TIRE_PRESSURE, ACCESS_READ, CHANGE_MODE_CONTINUOUS, 1.0, 50.0);
            tp_config.area_configs = vec![
                VehicleAreaConfig { area_id: 1, min_float_value: 0.0, max_float_value: 1000.0, ..Default::default() }, // LF
                VehicleAreaConfig { area_id: 2, min_float_value: 0.0, max_float_value: 1000.0, ..Default::default() }, // RF
                VehicleAreaConfig { area_id: 4, min_float_value: 0.0, max_float_value: 1000.0, ..Default::default() }, // LR
                VehicleAreaConfig { area_id: 8, min_float_value: 0.0, max_float_value: 1000.0, ..Default::default() }, // RR
            ];
            tx.send(Ok(tp_config)).await.unwrap();

            // 4. Current Gear (On Change)
            let gear_config = Self::create_prop_config(CURRENT_GEAR, ACCESS_READ, CHANGE_MODE_ON_CHANGE, 0.0, 0.0);
            tx.send(Ok(gear_config)).await.unwrap();

            // 5. Steering Angle (Continuous)
            let steering_config = Self::create_prop_config(PERF_STEERING_ANGLE, ACCESS_READ, CHANGE_MODE_CONTINUOUS, 10.0, 100.0);
            tx.send(Ok(steering_config)).await.unwrap();

            // 6. Battery Average Temp (Continuous)
            let batt_temp_config = Self::create_prop_config(EV_BATTERY_AVERAGE_TEMPERATURE, ACCESS_READ, CHANGE_MODE_CONTINUOUS, 1.0, 10.0);
            tx.send(Ok(batt_temp_config)).await.unwrap();
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }

    async fn set_values(
        &self,
        _request: Request<VehiclePropValueRequests>,
    ) -> Result<Response<SetValueResults>, Status> {
        Err(Status::unimplemented("Not yet implemented"))
    }

    async fn get_values(
        &self,
        _request: Request<VehiclePropValueRequests>,
    ) -> Result<Response<GetValueResults>, Status> {
        Err(Status::unimplemented("Not yet implemented"))
    }

    async fn update_sample_rate(
        &self,
        _request: Request<UpdateSampleRateRequest>,
    ) -> Result<Response<VehicleHalCallStatus>, Status> {
        Err(Status::unimplemented("Not yet implemented"))
    }

    async fn check_health(
        &self,
        _request: Request<()>,
    ) -> Result<Response<VehicleHalCallStatus>, Status> {
        Ok(Response::new(VehicleHalCallStatus {
            status_code: 0,
        }))
    }

    async fn dump(
        &self,
        _request: Request<DumpOptions>,
    ) -> Result<Response<DumpResult>, Status> {
        Err(Status::unimplemented("Not yet implemented"))
    }

    async fn start_property_values_stream(
        &self,
        _request: Request<()>,
    ) -> Result<Response<Self::StartPropertyValuesStreamStream>, Status> {
        let (tx, rx) = tokio::sync::mpsc::channel(128);
        
        tokio::task::spawn_blocking(move || {
            use helios_dds::{Participant, QosProfile, VehicleTelemetry, HELIOS_DOMAIN, TOPIC_TELEMETRY};
            use vehicle::VehiclePropValue;

            let p = match Participant::new(HELIOS_DOMAIN) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("Failed to create DDS participant: {}", e);
                    return;
                }
            };
            
            let r = match p.reader::<VehicleTelemetry>(TOPIC_TELEMETRY, QosProfile::Telemetry) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("Failed to create DDS reader: {}", e);
                    return;
                }
            };

            println!("Subscribed to {} on domain {}", TOPIC_TELEMETRY, HELIOS_DOMAIN);

            loop {
                match r.take_timeout(std::time::Duration::from_millis(100)) {
                    Ok(Some(telemetry)) => {
                        let mut values = Vec::new();
                        let timestamp = telemetry.t_ns as i64; // monotonic ns

                        // 1. Vehicle Speed (m/s)
                        values.push(VehiclePropValue {
                            timestamp,
                            area_id: 0,
                            prop: PERF_VEHICLE_SPEED,
                            status: 0,
                            float_values: vec![telemetry.speed_mps],
                            ..Default::default()
                        });

                        // 2. Battery SOC (%)
                        values.push(VehiclePropValue {
                            timestamp,
                            area_id: 0,
                            prop: EV_BATTERY_LEVEL,
                            status: 0,
                            float_values: vec![telemetry.soc_pct],
                            ..Default::default()
                        });

                        // 3. Current Gear
                        values.push(VehiclePropValue {
                            timestamp,
                            area_id: 0,
                            prop: CURRENT_GEAR,
                            status: 0,
                            int32_values: vec![telemetry.gear as i32],
                            ..Default::default()
                        });

                        // 4. Steering Angle (degrees)
                        values.push(VehiclePropValue {
                            timestamp,
                            area_id: 0,
                            prop: PERF_STEERING_ANGLE,
                            status: 0,
                            float_values: vec![telemetry.steer_road_deg],
                            ..Default::default()
                        });

                        // 5. Battery Temp
                        values.push(VehiclePropValue {
                            timestamp,
                            area_id: 0,
                            prop: EV_BATTERY_AVERAGE_TEMPERATURE,
                            status: 0,
                            float_values: vec![telemetry.cell_temp_c],
                            ..Default::default()
                        });

                        // 6. Tire Pressures (mocked from wheel slip/load or just mapped if we had TPMS)
                        // For demonstration, we'll just push standard tire pressures
                        let tp_areas = [1, 2, 4, 8];
                        for &area in &tp_areas {
                            values.push(VehiclePropValue {
                                timestamp,
                                area_id: area,
                                prop: TIRE_PRESSURE,
                                status: 0,
                                float_values: vec![240.0], // 240 kPa nominal
                                ..Default::default()
                            });
                        }

                        let msg = VehiclePropValues { values };
                        
                        // Send over gRPC stream. If receiver drops, break the loop.
                        if tx.blocking_send(Ok(msg)).is_err() {
                            println!("VHAL client disconnected.");
                            break;
                        }
                    }
                    Ok(None) => {
                        // Timeout, loop again
                    }
                    Err(e) => {
                        eprintln!("DDS read error: {:?}", e);
                        std::thread::sleep(std::time::Duration::from_millis(100));
                    }
                }
            }
        });
        
        Ok(Response::new(ReceiverStream::new(rx)))
    }

    async fn subscribe(
        &self,
        _request: Request<SubscribeRequest>,
    ) -> Result<Response<VehicleHalCallStatus>, Status> {
        Ok(Response::new(VehicleHalCallStatus {
            status_code: 0,
        }))
    }

    async fn unsubscribe(
        &self,
        _request: Request<UnsubscribeRequest>,
    ) -> Result<Response<VehicleHalCallStatus>, Status> {
        Ok(Response::new(VehicleHalCallStatus {
            status_code: 0,
        }))
    }

    async fn get_min_max_supported_values(
        &self,
        _request: Request<GetMinMaxSupportedValuesRequest>,
    ) -> Result<Response<GetMinMaxSupportedValuesResult>, Status> {
        Err(Status::unimplemented("Not yet implemented"))
    }

    async fn get_supported_values_lists(
        &self,
        _request: Request<GetSupportedValuesListsRequest>,
    ) -> Result<Response<GetSupportedValuesListsResult>, Status> {
        Err(Status::unimplemented("Not yet implemented"))
    }

    async fn start_supported_values_change_stream(
        &self,
        _request: Request<()>,
    ) -> Result<Response<Self::StartSupportedValuesChangeStreamStream>, Status> {
        let (_tx, rx) = tokio::sync::mpsc::channel(4);
        Ok(Response::new(ReceiverStream::new(rx)))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "0.0.0.0:50051".parse().unwrap();
    let service = HeliosVhalService::default();

    println!("HELIOS VHAL gRPC Server listening on {}", addr);

    Server::builder()
        .add_service(VehicleServerServer::new(service))
        .serve(addr)
        .await?;

    Ok(())
}
