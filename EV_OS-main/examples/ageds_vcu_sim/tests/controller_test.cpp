#include "ageds/controller.hpp"

#include <cassert>
#include <cmath>
#include <limits>

namespace {

ageds::SensorFrame valid_frame(std::uint64_t timestamp_ms) {
  ageds::SensorFrame frame{};
  frame.timestamp_ms = timestamp_ms;
  frame.imu_valid = true;
  frame.acceleration_mps2 = {0.0, 0.0, 35.0};
  frame.angular_rate_rps = {0.0, 0.0, 0.0};
  frame.ride_height_mm = {80.0, 81.0, 82.0, 83.0};
  frame.ride_height_valid.fill(true);
  for (std::size_t wheel = 0; wheel < ageds::kWheels; ++wheel) {
    frame.tire_ir_temperature_c[wheel].fill(55.0);
  }
  frame.tire_ir_valid.fill(true);
  frame.inverter_temperature_c = {60.0, 61.0, 62.0, 63.0};
  frame.inverter_valid.fill(true);
  frame.driver_controls_valid = true;
  frame.vehicle_speed_valid = true;
  frame.vehicle_speed_mps = 35.0;
  frame.steering_angle_rad = 0.0;
  return frame;
}

void thermal_override_takes_priority() {
  ageds::Controller controller;
  auto frame = valid_frame(100);
  controller.step(frame, 100);
  frame.timestamp_ms = 110;
  frame.inverter_temperature_c.fill(95.0);
  const auto commands = controller.step(frame, 110);
  assert(commands.fault == ageds::Fault::None);
  assert(commands.thermal_override);
  assert(commands.left_fan == 1.0);
  assert(commands.right_fan == 1.0);
  assert(commands.decklid_louvers == 1.0);
}

void corner_entry_preloads_inside_fan_and_flaps() {
  ageds::Controller controller;
  auto frame = valid_frame(100);
  frame.steering_angle_rad = 0.2;
  const auto commands = controller.step(frame, 100);
  assert(commands.corner_entry_preload);
  assert(commands.left_fan >= 0.75);
  assert(commands.right_fan < commands.left_fan);
  assert(commands.front_dive_planes >= 0.35);
  assert(commands.rear_gurney >= 0.35);
}

void dropout_is_reported_and_commands_fail_closed() {
  ageds::Controller controller;
  auto frame = valid_frame(100);
  frame.imu_valid = false;
  const auto commands = controller.step(frame, 100);
  assert(commands.fault == ageds::Fault::InvalidSensor);
  assert(commands.left_fan == 0.0);
  assert(commands.right_fan == 0.0);
  assert(commands.decklid_louvers == 1.0);
}

void stale_and_non_finite_frames_are_rejected() {
  ageds::Controller controller;
  auto stale = valid_frame(10);
  assert(controller.step(stale, 100).fault == ageds::Fault::StaleFrame);

  auto non_finite = valid_frame(100);
  non_finite.ride_height_mm[0] = std::numeric_limits<double>::quiet_NaN();
  assert(controller.step(non_finite, 100).fault ==
         ageds::Fault::NonFiniteSensor);
}

void state_estimate_is_synchronized_and_updated() {
  ageds::Controller controller;
  auto frame = valid_frame(100);
  frame.acceleration_mps2[1] = 2.0;
  controller.step(frame, 100);
  frame.timestamp_ms = 110;
  frame.acceleration_mps2[1] = 6.0;
  controller.step(frame, 110);
  assert(std::abs(controller.estimate().lateral_accel_mps2 - 3.0) < 1e-9);
}

}  // namespace

int main() {
  thermal_override_takes_priority();
  corner_entry_preloads_inside_fan_and_flaps();
  dropout_is_reported_and_commands_fail_closed();
  stale_and_non_finite_frames_are_rejected();
  state_estimate_is_synchronized_and_updated();
}
