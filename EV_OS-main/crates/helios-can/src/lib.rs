//! HELIOS CAN FD communication matrix.
//!
//! Single source of truth: [`MATRIX`]. Encoding, decoding, E2E protection and
//! the DBC export are all driven by the same table, so they cannot diverge.
//!
//! Bus topology (see docs/ARCHITECTURE.md, network section):
//! * PT-F / PT-R: CAN FD 1 Mbit/s arbitration, 5 Mbit/s data (CiA 601-4 SIC
//!   transceivers). One bus per axle so the 1 kHz torque loop stays < 35 % load.
//! * CH: chassis CAN FD (brake-by-wire, EPAS, IMU, aero actuators), 1/5 Mbit/s.
//! * BMS: BMS master <-> VCU, 500 kbit/s / 2 Mbit/s.
//! * BODY: classical CAN 500 kbit/s (Infinitybox, lighting, HMI buttons).
#![no_std]
#![forbid(unsafe_code)]

#[cfg(feature = "std")]
extern crate std;

pub mod crc {
    /// CRC-8 SAE J1850 (poly 0x1D, init 0xFF, xorout 0xFF). Check("123456789") = 0x4B.
    pub fn crc8_j1850(data: &[u8]) -> u8 {
        let mut c: u8 = 0xFF;
        for &b in data {
            c ^= b;
            for _ in 0..8 {
                c = if c & 0x80 != 0 { (c << 1) ^ 0x1D } else { c << 1 };
            }
        }
        c ^ 0xFF
    }

    /// CRC-16/CCITT-FALSE (poly 0x1021, init 0xFFFF, no reflection, xorout 0).
    /// Check("123456789") = 0x29B1. This is the AUTOSAR E2E Profile 5 CRC.
    pub fn crc16_ccitt_false_update(mut c: u16, data: &[u8]) -> u16 {
        for &b in data {
            c ^= (b as u16) << 8;
            for _ in 0..8 {
                c = if c & 0x8000 != 0 { (c << 1) ^ 0x1021 } else { c << 1 };
            }
        }
        c
    }
    pub fn crc16_ccitt_false(data: &[u8]) -> u16 {
        crc16_ccitt_false_update(0xFFFF, data)
    }
}

pub mod e2e {
    //! E2E protection, AUTOSAR Profile 5 layout: CRC16 (little-endian) at bytes 0..2,
    //! 8-bit counter at byte 2. CRC covers bytes 2..len, then Data ID low, Data ID high.
    use super::crc::crc16_ccitt_false_update;

    pub const HEADER_LEN: usize = 3;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Status {
        Ok,
        /// Counter unchanged: stale repetition.
        Repeated,
        /// Counter jumped by more than `max_delta`.
        WrongSequence,
        CrcError,
    }

    fn crc(buf: &[u8], data_id: u16) -> u16 {
        let c = crc16_ccitt_false_update(0xFFFF, &buf[2..]);
        crc16_ccitt_false_update(c, &data_id.to_le_bytes())
    }

    pub fn protect(buf: &mut [u8], data_id: u16, counter: u8) {
        buf[2] = counter;
        let c = crc(buf, data_id);
        buf[0..2].copy_from_slice(&c.to_le_bytes());
    }

    #[derive(Clone, Copy, Debug, Default)]
    pub struct Checker {
        last: Option<u8>,
        pub max_delta: u8,
    }

    impl Checker {
        pub const fn new(max_delta: u8) -> Self {
            Self { last: None, max_delta }
        }
        pub fn check(&mut self, buf: &[u8], data_id: u16) -> Status {
            let rx = u16::from_le_bytes([buf[0], buf[1]]);
            if rx != crc(buf, data_id) {
                return Status::CrcError;
            }
            let c = buf[2];
            let st = match self.last {
                None => Status::Ok,
                Some(l) => {
                    let d = c.wrapping_sub(l);
                    if d == 0 {
                        Status::Repeated
                    } else if d > self.max_delta {
                        Status::WrongSequence
                    } else {
                        Status::Ok
                    }
                }
            };
            self.last = Some(c);
            st
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bus {
    PtFront,
    PtRear,
    Chassis,
    Bms,
    Body,
}

impl Bus {
    pub const ALL: [Bus; 5] = [Bus::PtFront, Bus::PtRear, Bus::Chassis, Bus::Bms, Bus::Body];
    pub const fn name(self) -> &'static str {
        match self {
            Bus::PtFront => "PT_F",
            Bus::PtRear => "PT_R",
            Bus::Chassis => "CH",
            Bus::Bms => "BMS",
            Bus::Body => "BODY",
        }
    }
    /// (nominal bit rate, data bit rate, FD?)
    pub const fn rates(self) -> (u32, u32, bool) {
        match self {
            Bus::PtFront | Bus::PtRear | Bus::Chassis => (1_000_000, 5_000_000, true),
            Bus::Bms => (500_000, 2_000_000, true),
            Bus::Body => (500_000, 500_000, false),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    U8,
    I8,
    U16,
    I16,
    U32,
}

impl Kind {
    pub const fn bytes(self) -> usize {
        match self {
            Kind::U8 | Kind::I8 => 1,
            Kind::U16 | Kind::I16 => 2,
            Kind::U32 => 4,
        }
    }
    pub const fn signed(self) -> bool {
        matches!(self, Kind::I8 | Kind::I16)
    }
    pub const fn raw_range(self) -> (i64, i64) {
        match self {
            Kind::U8 => (0, 255),
            Kind::I8 => (-128, 127),
            Kind::U16 => (0, 65535),
            Kind::I16 => (-32768, 32767),
            Kind::U32 => (0, 4_294_967_295),
        }
    }
}

/// Little-endian (Intel) signal at a byte offset. physical = raw * factor + offset.
#[derive(Clone, Copy, Debug)]
pub struct Signal {
    pub name: &'static str,
    pub byte: usize,
    pub kind: Kind,
    pub factor: f32,
    pub offset: f32,
    pub unit: &'static str,
}

#[derive(Clone, Copy, Debug)]
pub struct Message {
    pub name: &'static str,
    pub id: u32,
    pub bus: Bus,
    /// Payload length in bytes (valid CAN FD length).
    pub len: usize,
    pub period_ms: u32,
    pub sender: &'static str,
    pub receivers: &'static str,
    /// E2E Data ID (unique per message).
    pub data_id: u16,
    pub signals: &'static [Signal],
}

const fn s(name: &'static str, byte: usize, kind: Kind, factor: f32, offset: f32, unit: &'static str) -> Signal {
    Signal { name, byte, kind, factor, offset, unit }
}

macro_rules! torque_cmd {
    ($name:literal, $id:expr, $bus:expr, $did:expr, $l:literal, $r:literal) => {
        Message {
            name: $name,
            id: $id,
            bus: $bus,
            len: 16,
            period_ms: 1,
            sender: "VCU",
            receivers: "INV",
            data_id: $did,
            signals: &[
                s(concat!("TrqCmd_", $l), 3, Kind::I16, 0.25, 0.0, "Nm"),
                s(concat!("TrqCmd_", $r), 5, Kind::I16, 0.25, 0.0, "Nm"),
                s("SpdLimit", 7, Kind::U16, 1.0, 0.0, "rpm"),
                s("EnableBits", 9, Kind::U8, 1.0, 0.0, ""),
                s("DriveMode", 10, Kind::U8, 1.0, 0.0, ""),
                s("TrqSlewLim", 11, Kind::U16, 10.0, 0.0, "Nm/ms"),
            ],
        }
    };
}

macro_rules! inv_status {
    ($name:literal, $id:expr, $bus:expr, $did:expr) => {
        Message {
            name: $name,
            id: $id,
            bus: $bus,
            len: 16,
            period_ms: 1,
            sender: "INV",
            receivers: "VCU",
            data_id: $did,
            signals: &[
                s("TrqAct", 3, Kind::I16, 0.25, 0.0, "Nm"),
                s("Speed", 5, Kind::I16, 0.5, 0.0, "rpm"),
                s("IdcAct", 7, Kind::I16, 0.1, 0.0, "A"),
                s("VdcAct", 9, Kind::U16, 0.1, 0.0, "V"),
                s("TWinding", 11, Kind::U8, 1.0, -40.0, "degC"),
                s("TPowerStage", 12, Kind::U8, 1.0, -40.0, "degC"),
                s("FaultCode", 13, Kind::U8, 1.0, 0.0, ""),
                s("InvState", 14, Kind::U8, 1.0, 0.0, ""),
            ],
        }
    };
}

/// The HELIOS communication matrix.
pub static MATRIX: &[Message] = &[
    Message {
        name: "HPC_AdvisoryCmd",
        id: 0x0E0,
        bus: Bus::Chassis,
        len: 16,
        period_ms: 10,
        sender: "HPC",
        receivers: "VCU",
        data_id: 0x00E0,
        signals: &[
            s("AdvTrq_FL", 3, Kind::I16, 0.25, 0.0, "Nm"),
            s("AdvTrq_FR", 5, Kind::I16, 0.25, 0.0, "Nm"),
            s("AdvTrq_RL", 7, Kind::I16, 0.25, 0.0, "Nm"),
            s("AdvTrq_RR", 9, Kind::I16, 0.25, 0.0, "Nm"),
            s("AdvSteerReq", 11, Kind::I16, 0.0001, 0.0, "rad"),
            s("AdvFlags", 13, Kind::U8, 1.0, 0.0, ""),
        ],
    },
    Message {
        name: "VCU_SupervisorStatus",
        id: 0x0E1,
        bus: Bus::Chassis,
        len: 8,
        period_ms: 10,
        sender: "VCU",
        receivers: "HPC",
        data_id: 0x00E1,
        signals: &[
            s("FirewallState", 3, Kind::U8, 1.0, 0.0, ""),
            s("ViolationBits", 4, Kind::U8, 1.0, 0.0, ""),
            s("ClampDeltaMax", 5, Kind::U16, 0.25, 0.0, "Nm"),
            s("AdvisoryActive", 7, Kind::U8, 1.0, 0.0, ""),
        ],
    },
    Message {
        name: "CCU_Heartbeat",
        id: 0x0F0,
        bus: Bus::Chassis,
        len: 8,
        period_ms: 10,
        sender: "CCU",
        receivers: "VCU",
        data_id: 0x00F0,
        signals: &[
            s("ReqDriveMode", 3, Kind::U8, 1.0, 0.0, ""),
            s("ReqTvEnable", 4, Kind::U8, 1.0, 0.0, ""),
            s("ReqAeroOverride", 5, Kind::U8, 1.0, 0.0, ""),
        ],
    },
    torque_cmd!("VCU_TrqCmd_F", 0x101, Bus::PtFront, 0x0101, "FL", "FR"),
    torque_cmd!("VCU_TrqCmd_R", 0x102, Bus::PtRear, 0x0102, "RL", "RR"),
    inv_status!("INV_FL_Status", 0x111, Bus::PtFront, 0x0111),
    inv_status!("INV_FR_Status", 0x112, Bus::PtFront, 0x0112),
    inv_status!("INV_RL_Status", 0x113, Bus::PtRear, 0x0113),
    inv_status!("INV_RR_Status", 0x114, Bus::PtRear, 0x0114),
    Message {
        name: "VCU_BrakeCmd",
        id: 0x120,
        bus: Bus::Chassis,
        len: 12,
        period_ms: 1,
        sender: "VCU",
        receivers: "BBW",
        data_id: 0x0120,
        signals: &[
            s("BrkTrq_FL", 3, Kind::U16, 1.0, 0.0, "Nm"),
            s("BrkTrq_FR", 5, Kind::U16, 1.0, 0.0, "Nm"),
            s("BrkTrq_RL", 7, Kind::U16, 1.0, 0.0, "Nm"),
            s("BrkTrq_RR", 9, Kind::U16, 1.0, 0.0, "Nm"),
            s("DecelReq", 11, Kind::U8, 0.1, 0.0, "m/s2"),
        ],
    },
    Message {
        name: "CH_Motion",
        id: 0x125,
        bus: Bus::Chassis,
        len: 16,
        period_ms: 1,
        sender: "IMU",
        receivers: "VCU",
        data_id: 0x0125,
        signals: &[
            s("Vx", 3, Kind::I16, 0.01, 0.0, "m/s"),
            s("Vy", 5, Kind::I16, 0.01, 0.0, "m/s"),
            s("YawRate", 7, Kind::I16, 0.0005, 0.0, "rad/s"),
            s("Ax", 9, Kind::I16, 0.005, 0.0, "m/s2"),
            s("Ay", 11, Kind::I16, 0.005, 0.0, "m/s2"),
            s("SteerRoad", 13, Kind::I16, 0.0001, 0.0, "rad"),
        ],
    },
    Message {
        name: "VCU_AeroCmd",
        id: 0x130,
        bus: Bus::Chassis,
        len: 8,
        period_ms: 10,
        sender: "VCU",
        receivers: "AERO",
        data_id: 0x0130,
        signals: &[s("AeroMode", 3, Kind::U8, 1.0, 0.0, ""), s("WingAngleCmd", 4, Kind::I16, 0.1, 0.0, "deg")],
    },
    Message {
        name: "VCU_Status",
        id: 0x180,
        bus: Bus::Chassis,
        len: 16,
        period_ms: 10,
        sender: "VCU",
        receivers: "CCU,BMS",
        data_id: 0x0180,
        signals: &[
            s("VehState", 3, Kind::U8, 1.0, 0.0, ""),
            s("Gear", 4, Kind::U8, 1.0, 0.0, ""),
            s("DriveModeAct", 5, Kind::U8, 1.0, 0.0, ""),
            s("FaultBits", 6, Kind::U32, 1.0, 0.0, ""),
            s("PowerScale", 10, Kind::U8, 0.005, 0.0, ""),
            s("ContactorCmd", 11, Kind::U8, 1.0, 0.0, ""),
            s("VehSpeed", 12, Kind::U16, 0.01, 0.0, "m/s"),
        ],
    },
    Message {
        name: "BMS_Status",
        id: 0x201,
        bus: Bus::Bms,
        len: 24,
        period_ms: 10,
        sender: "BMS",
        receivers: "VCU",
        data_id: 0x0201,
        signals: &[
            s("PackVoltage", 3, Kind::U16, 0.1, 0.0, "V"),
            s("PackCurrent", 5, Kind::I16, 0.1, 0.0, "A"),
            s("SOC", 7, Kind::U16, 0.01, 0.0, "%"),
            s("PDisLim", 9, Kind::U16, 1.0, 0.0, "kW"),
            s("PChgLim", 11, Kind::U16, 1.0, 0.0, "kW"),
            s("TCellMax", 13, Kind::U8, 1.0, -40.0, "degC"),
            s("TCellMin", 14, Kind::U8, 1.0, -40.0, "degC"),
            s("IsoRes", 15, Kind::U16, 1.0, 0.0, "kOhm"),
            s("Flags", 17, Kind::U8, 1.0, 0.0, ""),
            s("CellVMax", 18, Kind::U16, 0.001, 0.0, "V"),
            s("CellVMin", 20, Kind::U16, 0.001, 0.0, "V"),
            s("DcLinkVoltage", 22, Kind::U16, 0.1, 0.0, "V"),
        ],
    },
    Message {
        name: "BODY_Inputs",
        id: 0x300,
        bus: Bus::Body,
        len: 8,
        period_ms: 20,
        sender: "BODY",
        receivers: "VCU",
        data_id: 0x0300,
        signals: &[
            s("StartButton", 3, Kind::U8, 1.0, 0.0, ""),
            s("GearReq", 4, Kind::U8, 1.0, 0.0, ""),
            s("ModeReq", 5, Kind::U8, 1.0, 0.0, ""),
            s("Wake", 6, Kind::U8, 1.0, 0.0, ""),
        ],
    },
];

pub fn by_name(name: &str) -> Option<&'static Message> {
    MATRIX.iter().find(|m| m.name == name)
}
pub fn by_id(bus: Bus, id: u32) -> Option<&'static Message> {
    MATRIX.iter().find(|m| m.id == id && m.bus == bus)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CodecError {
    Length,
    Range { signal: usize },
    E2e(e2e::Status),
}

/// Encode physical values (one per signal, in table order) into `buf[..msg.len]`, then
/// apply E2E protection with `counter`.
pub fn encode(msg: &Message, values: &[f32], counter: u8, buf: &mut [u8]) -> Result<(), CodecError> {
    if values.len() != msg.signals.len() || buf.len() < msg.len {
        return Err(CodecError::Length);
    }
    buf[..msg.len].fill(0);
    for (k, (sig, v)) in msg.signals.iter().zip(values).enumerate() {
        let raw = libm_round((v - sig.offset) / sig.factor);
        let (lo, hi) = sig.kind.raw_range();
        if !(raw >= lo as f64 && raw <= hi as f64) {
            return Err(CodecError::Range { signal: k });
        }
        let r = raw as i64;
        let bytes = (r as u64).to_le_bytes();
        buf[sig.byte..sig.byte + sig.kind.bytes()].copy_from_slice(&bytes[..sig.kind.bytes()]);
    }
    e2e::protect(&mut buf[..msg.len], msg.data_id, counter);
    Ok(())
}

/// Decode `buf` into physical values after an E2E check.
pub fn decode(msg: &Message, buf: &[u8], checker: &mut e2e::Checker, out: &mut [f32]) -> Result<(), CodecError> {
    if buf.len() < msg.len || out.len() != msg.signals.len() {
        return Err(CodecError::Length);
    }
    match checker.check(&buf[..msg.len], msg.data_id) {
        e2e::Status::Ok => {}
        st => return Err(CodecError::E2e(st)),
    }
    for (sig, o) in msg.signals.iter().zip(out.iter_mut()) {
        let n = sig.kind.bytes();
        let mut b = [0u8; 8];
        b[..n].copy_from_slice(&buf[sig.byte..sig.byte + n]);
        let u = u64::from_le_bytes(b);
        let raw: i64 = match sig.kind {
            Kind::I8 => (u as u8) as i8 as i64,
            Kind::I16 => (u as u16) as i16 as i64,
            _ => u as i64,
        };
        *o = raw as f32 * sig.factor + sig.offset;
    }
    Ok(())
}

fn libm_round(x: f32) -> f64 {
    let x = x as f64;
    if x >= 0.0 { (x + 0.5) as i64 as f64 } else { -((-x + 0.5) as i64 as f64) }
}

pub mod busload {
    //! Worst-case frame transmission time per ISO 11898-1:2015 (11-bit identifiers).
    //!
    //! Classical CAN: bits = 47 + 8n + floor((34 + 8n - 1)/4)        (Davis et al. 2007)
    //! CAN FD (BRS=1):
    //!   nominal phase: SOF..BRS (17 bits) + worst stuff floor(16/4)=4,
    //!                  CRC delimiter, ACK, ACK delimiter, EOF(7), IFS(3) = 13 bits
    //!   data phase:    ESI + DLC (5) + 8n data, dynamic stuff floor((5 + 8n - 1)/4),
    //!                  stuff count (4) + CRC (17 if n <= 16 else 21) + fixed stuff
    //!                  bits ceil((4 + CRC)/4)
    use super::{Bus, MATRIX};

    pub fn classic_frame_us(n: usize, bps: u32) -> f64 {
        let n = n as u32;
        let bits = 47 + 8 * n + (34 + 8 * n - 1) / 4;
        bits as f64 * 1e6 / bps as f64
    }

    pub fn fd_frame_us(n: usize, nominal_bps: u32, data_bps: u32) -> f64 {
        let n = n as u32;
        let nominal_bits = 17 + 4 + 13;
        let crc = if n <= 16 { 17 } else { 21 };
        let data_bits = 5 + 8 * n + (5 + 8 * n - 1) / 4 + 4 + crc + (4 + crc).div_ceil(4);
        nominal_bits as f64 * 1e6 / nominal_bps as f64 + data_bits as f64 * 1e6 / data_bps as f64
    }

    pub fn frame_us(bus: Bus, n: usize) -> f64 {
        let (nom, data, fd) = bus.rates();
        if fd { fd_frame_us(n, nom, data) } else { classic_frame_us(n, nom) }
    }

    /// Worst-case utilisation (0..1) of `bus` from the matrix.
    pub fn utilisation(bus: Bus) -> f64 {
        MATRIX
            .iter()
            .filter(|m| m.bus == bus)
            .map(|m| frame_us(bus, m.len) / (m.period_ms as f64 * 1000.0))
            .sum()
    }
}

#[cfg(feature = "std")]
pub mod dbc {
    //! Vector DBC export (CAN FD via VFrameFormat attribute). Intel byte order.
    use super::{Bus, MATRIX};
    use std::fmt::Write;
    use std::string::String;

    pub fn export(bus: Bus) -> String {
        let mut o = String::new();
        let _ = writeln!(o, "VERSION \"HELIOS {} generated by helios-can\"\n", bus.name());
        let _ = writeln!(o, "NS_ :\n\nBS_:\n");
        let mut nodes: std::vec::Vec<&str> = std::vec::Vec::new();
        for m in MATRIX.iter().filter(|m| m.bus == bus) {
            for n in core::iter::once(m.sender).chain(m.receivers.split(',')) {
                if !nodes.contains(&n) {
                    nodes.push(n);
                }
            }
        }
        let _ = writeln!(o, "BU_: {}\n", nodes.join(" "));
        for m in MATRIX.iter().filter(|m| m.bus == bus) {
            let _ = writeln!(o, "BO_ {} {}: {} {}", m.id, m.name, m.len, m.sender);
            let _ = writeln!(o, " SG_ E2E_CRC : 0|16@1+ (1,0) [0|65535] \"\" {}", m.receivers);
            let _ = writeln!(o, " SG_ E2E_Counter : 16|8@1+ (1,0) [0|255] \"\" {}", m.receivers);
            for s in m.signals {
                let bits = s.kind.bytes() * 8;
                let sign = if s.kind.signed() { '-' } else { '+' };
                let (lo, hi) = s.kind.raw_range();
                let pmin = lo as f64 * s.factor as f64 + s.offset as f64;
                let pmax = hi as f64 * s.factor as f64 + s.offset as f64;
                let _ = writeln!(
                    o,
                    " SG_ {} : {}|{}@1{} ({},{}) [{}|{}] \"{}\" {}",
                    s.name,
                    s.byte * 8,
                    bits,
                    sign,
                    s.factor,
                    s.offset,
                    pmin,
                    pmax,
                    s.unit,
                    m.receivers
                );
            }
            let _ = writeln!(o);
        }
        let (_, _, fd) = bus.rates();
        let _ = writeln!(o, "BA_DEF_ BO_ \"GenMsgCycleTime\" INT 0 65535;");
        let _ = writeln!(o, "BA_DEF_ BO_ \"VFrameFormat\" ENUM \"StandardCAN\",\"ExtendedCAN\",\"reserved\",\"reserved\",\"reserved\",\"reserved\",\"reserved\",\"reserved\",\"reserved\",\"reserved\",\"reserved\",\"reserved\",\"reserved\",\"reserved\",\"StandardCAN_FD\",\"ExtendedCAN_FD\";");
        let _ = writeln!(o, "BA_DEF_ BU_ \"E2EDataId\" INT 0 65535;");
        let _ = writeln!(o, "BA_DEF_DEF_ \"GenMsgCycleTime\" 0;");
        let _ = writeln!(o, "BA_DEF_DEF_ \"VFrameFormat\" \"StandardCAN\";");
        let _ = writeln!(o, "BA_DEF_DEF_ \"E2EDataId\" 0;");
        for m in MATRIX.iter().filter(|m| m.bus == bus) {
            let _ = writeln!(o, "BA_ \"GenMsgCycleTime\" BO_ {} {};", m.id, m.period_ms);
            if fd {
                let _ = writeln!(o, "BA_ \"VFrameFormat\" BO_ {} 14;", m.id);
            }
        }
        o
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc_check_values() {
        assert_eq!(crc::crc8_j1850(b"123456789"), 0x4B);
        assert_eq!(crc::crc16_ccitt_false(b"123456789"), 0x29B1);
    }

    #[test]
    fn matrix_is_well_formed() {
        const FD_LENS: [usize; 16] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 12, 16, 20, 24, 32, 48, 64];
        for (i, m) in MATRIX.iter().enumerate() {
            let (_, _, fd) = m.bus.rates();
            assert!(if fd { FD_LENS.contains(&m.len) } else { m.len <= 8 }, "{} len", m.name);
            let mut used = [false; 64];
            used[0] = true;
            used[1] = true;
            used[2] = true;
            for s in m.signals {
                for b in s.byte..s.byte + s.kind.bytes() {
                    assert!(b < m.len, "{}.{} beyond payload", m.name, s.name);
                    assert!(!used[b], "{}.{} overlaps", m.name, s.name);
                    used[b] = true;
                }
            }
            for n in MATRIX.iter().skip(i + 1) {
                assert!(!(n.id == m.id && n.bus == m.bus), "duplicate id {:#x}", m.id);
                assert_ne!(n.data_id, m.data_id, "duplicate E2E data id");
            }
        }
    }

    #[test]
    fn roundtrip_and_e2e() {
        let m = by_name("VCU_TrqCmd_R").expect("msg");
        let mut buf = [0u8; 64];
        let vals = [-1234.25, 4299.75, 2800.0, 0b1011 as f32, 2.0, 50.0];
        encode(m, &vals, 7, &mut buf).expect("enc");
        let mut chk = e2e::Checker::new(2);
        let mut out = [0.0f32; 6];
        decode(m, &buf, &mut chk, &mut out).expect("dec");
        for (a, b) in vals.iter().zip(out.iter()) {
            assert!((a - b).abs() <= 0.125 + 1e-3, "{a} vs {b}");
        }
        // Repetition detected
        assert_eq!(decode(m, &buf, &mut chk, &mut out), Err(CodecError::E2e(e2e::Status::Repeated)));
        // Bit flip detected
        encode(m, &vals, 8, &mut buf).expect("enc");
        buf[5] ^= 0x10;
        assert_eq!(decode(m, &buf, &mut chk, &mut out), Err(CodecError::E2e(e2e::Status::CrcError)));
        // Same payload from a different message (masquerade) fails on Data ID
        let other = by_name("VCU_TrqCmd_F").expect("msg");
        encode(other, &vals, 9, &mut buf).expect("enc");
        assert_eq!(decode(m, &buf, &mut chk, &mut out), Err(CodecError::E2e(e2e::Status::CrcError)));
    }

    #[test]
    fn range_violation_rejected() {
        let m = by_name("VCU_TrqCmd_F").expect("msg");
        let mut buf = [0u8; 64];
        let vals = [9000.0, 0.0, 0.0, 0.0, 0.0, 0.0]; // > 8191.75 Nm at 0.25 Nm/bit
        assert_eq!(encode(m, &vals, 0, &mut buf), Err(CodecError::Range { signal: 0 }));
    }

    #[test]
    fn fd_frame_time_reference_values() {
        // 16-byte FD frame: nominal 34 bits; data 5+128+33+4+17+6 = 193 bits.
        let t = busload::fd_frame_us(16, 1_000_000, 5_000_000);
        assert!((t - (34.0 + 193.0 / 5.0)).abs() < 1e-9, "{t}");
        // Classical 8-byte frame at 500 kbit/s: 47 + 64 + 24 = 135 bits = 270 us.
        assert!((busload::classic_frame_us(8, 500_000) - 270.0).abs() < 1e-9);
    }

    #[test]
    fn bus_load_within_design_limit() {
        for b in Bus::ALL {
            let u = busload::utilisation(b);
            assert!(u < 0.40, "{} at {:.1} %", b.name(), u * 100.0);
        }
    }
}
