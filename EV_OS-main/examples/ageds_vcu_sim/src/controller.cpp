#include "ageds/controller.hpp"

#include <algorithm>
#include <cmath>
#include <limits>

namespace ageds {
namespace {

constexpr std::array<double, 5> kCandidates{0.0, 0.25, 0.5, 0.75, 1.0};
constexpr std::size_t kHorizon = 5;
constexpr double kFilterWeight = 0.25;
constexpr double kDropoutSafeLouvers = 1.0;

bool finite(double value) { return std::isfinite(value); }

template <std::size_t N>
bool all_finite(const std::array<double, N>& values) {
  return std::all_of(values.begin(), values.end(), finite);
}

}  // namespace

Controller::Controller(Calibration calibration) : calibration_(calibration) {}

bool Controller::calibration_valid() const {
  return calibration_.max_frame_age_ms > 0 &&
         finite(calibration_.inverter_override_c) &&
         finite(calibration_.corner_entry_steering_rad) &&
         calibration_.corner_entry_steering_rad > 0.0 &&
         finite(calibration_.corner_entry_lateral_accel_mps2) &&
         calibration_.corner_entry_lateral_accel_mps2 > 0.0 &&
         finite(calibration_.wheelbase_m) && calibration_.wheelbase_m > 0.0;
}

bool Controller::frame_valid(const SensorFrame& frame, std::uint64_t now_ms,
                             Fault& fault) const {
  if (!calibration_valid()) {
    fault = Fault::InvalidCalibration;
    return false;
  }
  if (frame.timestamp_ms > now_ms) {
    fault = Fault::FutureTimestamp;
    return false;
  }
  if (now_ms - frame.timestamp_ms > calibration_.max_frame_age_ms) {
    fault = Fault::StaleFrame;
    return false;
  }

  const bool valid_arrays =
      std::all_of(frame.ride_height_valid.begin(), frame.ride_height_valid.end(),
                  [](bool valid) { return valid; }) &&
      std::all_of(frame.tire_ir_valid.begin(), frame.tire_ir_valid.end(),
                  [](bool valid) { return valid; }) &&
      std::all_of(frame.inverter_valid.begin(), frame.inverter_valid.end(),
                  [](bool valid) { return valid; });
  if (!frame.imu_valid || !valid_arrays || !frame.driver_controls_valid ||
      !frame.vehicle_speed_valid) {
    fault = Fault::InvalidSensor;
    return false;
  }

  bool tire_values_finite = true;
  for (const auto& tire : frame.tire_ir_temperature_c) {
    tire_values_finite =
        tire_values_finite && all_finite(tire);
  }
  if (!all_finite(frame.acceleration_mps2) ||
      !all_finite(frame.angular_rate_rps) ||
      !all_finite(frame.ride_height_mm) || !tire_values_finite ||
      !all_finite(frame.inverter_temperature_c) ||
      !finite(frame.vehicle_speed_mps) || frame.vehicle_speed_mps < 0.0 ||
      !finite(frame.steering_angle_rad) || !finite(frame.throttle_position) ||
      !finite(frame.brake_position) || frame.throttle_position < 0.0 ||
      frame.throttle_position > 1.0 || frame.brake_position < 0.0 ||
      frame.brake_position > 1.0) {
    fault = Fault::NonFiniteSensor;
    return false;
  }

  fault = Fault::None;
  return true;
}

SimulatedCommands Controller::fail_closed(Fault fault) {
  last_commands_ = {};
  last_commands_.decklid_louvers = kDropoutSafeLouvers;
  last_commands_.fault = fault;
  return last_commands_;
}

double Controller::predictive_search(double target, double previous) const {
  double best_command = 0.0;
  double best_cost = std::numeric_limits<double>::infinity();
  for (double candidate : kCandidates) {
    double predicted = previous;
    double cost = 0.0;
    for (std::size_t step = 0; step < kHorizon; ++step) {
      predicted += 0.5 * (candidate - predicted);
      const double error = target - predicted;
      cost += error * error + 0.02 * candidate * candidate;
    }
    const double change = candidate - previous;
    cost += 0.1 * change * change;
    if (cost < best_cost) {
      best_cost = cost;
      best_command = candidate;
    }
  }
  return best_command;
}

SimulatedCommands Controller::step(const SensorFrame& frame,
                                   std::uint64_t now_ms) {
  std::lock_guard<std::mutex> lock(mutex_);
  Fault fault = Fault::None;
  if (!frame_valid(frame, now_ms, fault)) {
    return fail_closed(fault);
  }

  double mean_height = 0.0;
  double mean_tire_temperature = 0.0;
  for (std::size_t wheel = 0; wheel < kWheels; ++wheel) {
    mean_height += frame.ride_height_mm[wheel];
    for (double temperature : frame.tire_ir_temperature_c[wheel]) {
      mean_tire_temperature += temperature;
    }
  }
  mean_height /= static_cast<double>(kWheels);
  mean_tire_temperature /=
      static_cast<double>(kWheels * kIrElementsPerTire);
  const double max_inverter_temperature =
      *std::max_element(frame.inverter_temperature_c.begin(),
                        frame.inverter_temperature_c.end());

  EstimatedState measured{
      frame.acceleration_mps2[0],
      frame.acceleration_mps2[1],
      frame.angular_rate_rps[2],
      mean_height,
      mean_tire_temperature,
      max_inverter_temperature,
  };
  if (have_estimate_) {
    estimate_.longitudinal_accel_mps2 +=
        kFilterWeight *
        (measured.longitudinal_accel_mps2 - estimate_.longitudinal_accel_mps2);
    estimate_.lateral_accel_mps2 +=
        kFilterWeight *
        (measured.lateral_accel_mps2 - estimate_.lateral_accel_mps2);
    estimate_.yaw_rate_rps +=
        kFilterWeight * (measured.yaw_rate_rps - estimate_.yaw_rate_rps);
    estimate_.mean_ride_height_mm +=
        kFilterWeight * (measured.mean_ride_height_mm -
                         estimate_.mean_ride_height_mm);
    estimate_.mean_tire_temperature_c +=
        kFilterWeight * (measured.mean_tire_temperature_c -
                         estimate_.mean_tire_temperature_c);
    estimate_.max_inverter_temperature_c =
        kFilterWeight * measured.max_inverter_temperature_c +
        (1.0 - kFilterWeight) * estimate_.max_inverter_temperature_c;
  } else {
    estimate_ = measured;
    have_estimate_ = true;
  }

  const bool thermal_override =
      max_inverter_temperature >= calibration_.inverter_override_c;
  const bool turning =
      std::abs(frame.steering_angle_rad) >=
      calibration_.corner_entry_steering_rad;
  const double predicted_lateral_accel =
      frame.vehicle_speed_mps * frame.vehicle_speed_mps *
      std::tan(frame.steering_angle_rad) / calibration_.wheelbase_m;
  const bool predicted_high_g =
      std::abs(predicted_lateral_accel) >=
      calibration_.corner_entry_lateral_accel_mps2;
  const bool lateral_load_started =
      std::abs(frame.acceleration_mps2[1]) >=
      calibration_.corner_entry_lateral_accel_mps2;
  const bool corner_entry = turning && predicted_high_g && !lateral_load_started;

  SimulatedCommands commands{};
  commands.thermal_override = thermal_override;
  commands.corner_entry_preload = corner_entry;
  commands.fault = Fault::None;

  if (thermal_override) {
    commands.left_fan = 1.0;
    commands.right_fan = 1.0;
    commands.decklid_louvers = 1.0;
  } else {
    const double speed_proxy =
        std::clamp(frame.vehicle_speed_mps / 80.0, 0.0, 1.0);
    const double tire_heat_proxy = std::clamp(
        (estimate_.mean_tire_temperature_c - 40.0) / 80.0, 0.0, 1.0);
    const double fan_target =
        std::clamp(0.15 + 0.55 * speed_proxy + 0.2 * tire_heat_proxy, 0.0,
                   1.0);
    const double fan = predictive_search(fan_target, last_commands_.left_fan);
    commands.left_fan = fan;
    commands.right_fan = fan;
    commands.decklid_louvers = std::clamp(0.2 + 0.6 * tire_heat_proxy, 0.0, 1.0);
    commands.front_dive_planes =
        std::clamp(0.2 + 0.5 * speed_proxy, 0.0, 1.0);
    commands.rear_gurney =
        std::clamp(0.2 + 0.5 * speed_proxy, 0.0, 1.0);

    if (corner_entry) {
      const bool steering_left = frame.steering_angle_rad > 0.0;
      const double inside_fan = std::max(fan, 0.75);
      if (steering_left) {
        commands.left_fan = inside_fan;
      } else {
        commands.right_fan = inside_fan;
      }
      commands.front_dive_planes =
          std::max(commands.front_dive_planes, 0.35);
      commands.rear_gurney = std::max(commands.rear_gurney, 0.35);
    }
  }

  last_commands_ = commands;
  return commands;
}

EstimatedState Controller::estimate() const {
  std::lock_guard<std::mutex> lock(mutex_);
  return estimate_;
}

}  // namespace ageds
