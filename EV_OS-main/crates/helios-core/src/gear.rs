//! Gear selector and fault bookkeeping.

/// Driver gear selection. Values are the AOSP `VehicleGear` bit values so the
/// VHAL bridge forwards them without a lookup table.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[repr(i32)]
pub enum Gear {
    Neutral = 0x0001,
    Reverse = 0x0002,
    #[default]
    Park = 0x0004,
    Drive = 0x0008,
}

impl Gear {
    pub const fn aosp(self) -> i32 {
        self as i32
    }
    pub fn from_aosp(v: i32) -> Option<Self> {
        match v {
            0x0001 => Some(Gear::Neutral),
            0x0002 => Some(Gear::Reverse),
            0x0004 => Some(Gear::Park),
            0x0008 => Some(Gear::Drive),
            _ => None,
        }
    }
    pub const fn propulsive(self) -> bool {
        matches!(self, Gear::Drive | Gear::Reverse)
    }
}
