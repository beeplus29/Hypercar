//! Fault catalogue. Bit positions are part of the external interface (CAN
//! `VCU_State.FaultBits`, DDS `VehicleStatus.fault_bits`, VHAL
//! `HELIOS_FAULT_BITS`) and must never be renumbered — append only.

/// Reaction class, ordered by severity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Reaction {
    /// Inform driver only.
    Warn,
    /// Stay in Drive with reduced power/torque.
    Degrade,
    /// Controlled stop, then HV shutdown.
    SafeStop,
    /// Ramp current down, open contactors immediately.
    HvOff,
}

macro_rules! faults {
    ($( $(#[$m:meta])* $name:ident = $bit:expr => $react:ident, $latched:expr; )*) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        #[repr(u8)]
        pub enum Fault { $( $(#[$m])* $name = $bit, )* }

        impl Fault {
            pub const ALL: &'static [Fault] = &[$(Fault::$name,)*];
            pub const fn reaction(self) -> Reaction {
                match self { $( Fault::$name => Reaction::$react, )* }
            }
            /// Latched faults need a service reset (`VsmInputs::service_reset`) at standstill.
            pub const fn latched(self) -> bool {
                match self { $( Fault::$name => $latched, )* }
            }
            pub const fn name(self) -> &'static str {
                match self { $( Fault::$name => stringify!($name), )* }
            }
        }
    };
}

faults! {
    /// High-voltage interlock loop open (connector unplugged / service disconnect).
    HvilOpen = 0 => HvOff, false;
    /// Insulation resistance below 500 ohm/V (UN R100 combined AC+DC threshold).
    IsolationFault = 1 => Degrade, false;
    /// Insulation resistance below the 1000 ohm/V early-warning threshold.
    IsolationWarning = 2 => Warn, false;
    /// Crash signal from the airbag control unit.
    Crash = 3 => HvOff, true;
    /// DC-link failed to reach 95 % of pack voltage within the precharge window.
    PrechargeTimeout = 4 => HvOff, true;
    /// DC-link rose faster than the RC model allows: main+ welded or DC-link open.
    PrechargeTooFast = 5 => HvOff, true;
    /// DC-link stayed above 60 V after the active discharge window.
    DischargeTimeout = 6 => HvOff, true;
    /// Contactor weld detected (DC-link tracks pack voltage with contactors commanded open).
    ContactorWeld = 7 => HvOff, true;
    /// BMS critical: cell over/under-voltage, over-temperature, over-current.
    BmsCritical = 8 => SafeStop, true;
    /// BMS warning: approaching a cell limit; power is derated.
    BmsWarning = 9 => Degrade, false;
    MotorFaultFL = 10 => Degrade, false;
    MotorFaultFR = 11 => Degrade, false;
    MotorFaultRL = 12 => Degrade, false;
    MotorFaultRR = 13 => Degrade, false;
    /// Two or more motors lost: not enough actuator authority to guarantee yaw stability.
    MultiMotorLoss = 14 => SafeStop, false;
    /// Central compute (S32G3) heartbeat older than 100 ms.
    CentralHeartbeatStale = 15 => Degrade, false;
    /// Central compute heartbeat older than 500 ms.
    CentralHeartbeatLost = 16 => SafeStop, false;
    /// 12 V supply below 10.5 V.
    LvUndervoltage = 17 => Degrade, false;
    /// 12 V supply below 9.0 V: control electronics brown-out imminent.
    LvCritical = 18 => SafeStop, false;
    /// Charge connector detected while moving (sensor implausible).
    ChargeConnectorImplausible = 19 => Warn, false;
    /// Pedal plausibility: accelerator and brake pressed together above thresholds.
    PedalConflict = 20 => Degrade, false;
    /// Motor/battery thermal derate active.
    ThermalDerate = 21 => Warn, false;
    /// Steering angle sensor channels disagree.
    SteeringSensorFault = 22 => Degrade, false;
    /// Ground-speed (optical) sensor invalid; wheel-speed fallback in use.
    GroundSpeedInvalid = 23 => Degrade, false;
}

/// Bit set of active faults.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FaultSet(pub u64);

impl FaultSet {
    pub const EMPTY: FaultSet = FaultSet(0);

    #[inline]
    pub fn set(&mut self, f: Fault, on: bool) {
        let m = 1u64 << (f as u8);
        if on {
            self.0 |= m;
        } else {
            self.0 &= !m;
        }
    }
    #[inline]
    pub fn contains(&self, f: Fault) -> bool {
        self.0 & (1u64 << (f as u8)) != 0
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0 == 0
    }
    pub fn union(self, o: FaultSet) -> FaultSet {
        FaultSet(self.0 | o.0)
    }
    /// Highest reaction among the active faults.
    pub fn worst(&self) -> Option<Reaction> {
        Fault::ALL.iter().filter(|f| self.contains(**f)).map(|f| f.reaction()).max()
    }
    pub fn iter(&self) -> impl Iterator<Item = Fault> + '_ {
        Fault::ALL.iter().copied().filter(move |f| self.contains(*f))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bits_are_unique_and_dense() {
        let mut seen = 0u64;
        for (i, f) in Fault::ALL.iter().enumerate() {
            let b = 1u64 << (*f as u8);
            assert_eq!(seen & b, 0, "duplicate bit {}", f.name());
            assert_eq!(*f as usize, i, "fault bits must be dense and ordered");
            seen |= b;
        }
    }

    #[test]
    fn worst_reaction() {
        let mut s = FaultSet::EMPTY;
        assert_eq!(s.worst(), None);
        s.set(Fault::IsolationWarning, true);
        s.set(Fault::MotorFaultFL, true);
        assert_eq!(s.worst(), Some(Reaction::Degrade));
        s.set(Fault::Crash, true);
        assert_eq!(s.worst(), Some(Reaction::HvOff));
    }
}
