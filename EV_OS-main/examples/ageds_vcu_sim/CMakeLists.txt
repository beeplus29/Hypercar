cmake_minimum_required(VERSION 3.16)
project(ageds_vcu_sim LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)

add_library(ageds_controller src/controller.cpp)
target_include_directories(ageds_controller PUBLIC include)
target_compile_options(ageds_controller PRIVATE -Wall -Wextra -Wpedantic -Werror)

add_executable(ageds_controller_test tests/controller_test.cpp)
target_link_libraries(ageds_controller_test PRIVATE ageds_controller)
target_compile_options(ageds_controller_test PRIVATE -Wall -Wextra -Wpedantic -Werror)

enable_testing()
add_test(NAME ageds_controller_test COMMAND ageds_controller_test)
