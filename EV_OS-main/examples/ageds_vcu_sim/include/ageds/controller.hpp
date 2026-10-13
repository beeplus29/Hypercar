#pragma once

#include <array>
#include <cstdint>
#include <mutex>

namespace ageds {

inline constexpr std::uint32_t kControlPeriodMs = 10;
inline constexpr std::size_t kWheels = 4;
inline constexpr std::size_t kIrElementsPerTire = 8;
inline constexpr std::size_t kInverters = 4;

struct SensorFrame {
  std::uint64_t timestamp_ms{};
  bool imu_valid{};
  std::array<double, 3> acceleration_mps2{};
  std::array<double, 3> angular_rate_rps{};
  std::array<double, kWheels> ride_height_mm{};
  std::array<bool, kWheels> ride_height_valid{};
  std::array<std::array<double, kIrElementsPerTire>, kWheels>
      tire_ir_temperature_c{};
  std::array<bool, kWheels> tire_ir_valid{};
  std::array<double, kInverters> inverter_temperature_c{};
  std::array<bool, kInverters> inverter_valid{};
  bool driver_controls_valid{};
  bool vehicle_speed_valid{};
  double vehicle_speed_mps{};
  double steering_angle_rad{};
  double throttle_position{};
  double brake_position{};
};

struct Calibration {
  std::uint32_t max_frame_age_ms{30};
  double inverter_override_c{90.0};
  double corner_entry_steering_rad{0.08};
  double corner_entry_lateral_accel_mps2{1.0};
  double wheelbase_m{2.8};
};

enum class Fault : std::uint8_t {
  None,
  FutureTimestamp,
  StaleFrame,
  InvalidSensor,
  NonFiniteSensor,
  InvalidCalibration,
};

struct SimulatedCommands {
  double left_fan{};
  double right_fan{};
  double front_dive_planes{};
  double rear_gurney{};
  double decklid_louvers{};
  bool thermal_override{};
  bool corner_entry_preload{};
  Fault fault{Fault::None};
};

struct EstimatedState {
  double longitudinal_accel_mps2{};
  double lateral_accel_mps2{};
  double yaw_rate_rps{};
  double mean_ride_height_mm{};
  double mean_tire_temperature_c{};
  double max_inverter_temperature_c{};
};

class Controller {
 public:
  explicit Controller(Calibration calibration = {});

  SimulatedCommands step(const SensorFrame& frame, std::uint64_t now_ms);
  EstimatedState estimate() const;

 private:
  bool calibration_valid() const;
  bool frame_valid(const SensorFrame& frame, std::uint64_t now_ms,
                   Fault& fault) const;
  SimulatedCommands fail_closed(Fault fault);
  double predictive_search(double target, double previous) const;

  Calibration calibration_;
  mutable std::mutex mutex_;
  EstimatedState estimate_{};
  SimulatedCommands last_commands_{};
  bool have_estimate_{};
};

}  // namespace ageds
