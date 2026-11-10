# libvmcu/tests/CMakeLists.txt — Tests de libvmcu et du shim STM32 HAL.
#
# Chaque test lance une fausse app (fake_app.c) dans un fil du processus :
# le protocole réel passe par un socket Unix (macOS, Linux) ou un named pipe
# (Windows).

add_library(vmcu_fake_app STATIC fake_app.c)
target_include_directories(vmcu_fake_app
  PUBLIC  ${CMAKE_CURRENT_SOURCE_DIR}
          ${PROJECT_SOURCE_DIR}/core
          ${PROJECT_SOURCE_DIR}/platform
)
target_link_libraries(vmcu_fake_app PUBLIC vmcu)
target_compile_options(vmcu_fake_app PRIVATE ${VMCU_WARNING_FLAGS})

foreach(_test test_wire test_client)
  add_executable(${_test} ${_test}.c)
  target_link_libraries(${_test} PRIVATE vmcu_fake_app vmcu)
  target_compile_options(${_test} PRIVATE ${VMCU_WARNING_FLAGS})
  add_test(NAME ${_test} COMMAND ${_test})
endforeach()

foreach(_test test_stm32_hal test_stm32_hal_stubs)
  add_executable(${_test} ${_test}.c)
  target_link_libraries(${_test} PRIVATE vmcu_fake_app prebench_stm32_hal vmcu)
  target_compile_options(${_test} PRIVATE ${VMCU_WARNING_FLAGS})
  add_test(NAME ${_test} COMMAND ${_test})
endforeach()

# Projet STM32CubeMX du dépôt, compilé tel quel ; seul `main` est renommé.
set(PREBENCH_CUBEMX_TEST_PROJECT "${PROJECT_SOURCE_DIR}/../tests/cubemx-nucleo-f401re")
prebench_cubemx_sources(_cubemx_srcs "${PREBENCH_CUBEMX_TEST_PROJECT}")
add_library(cubemx_nucleo_f401re OBJECT ${_cubemx_srcs})
prebench_configure_cubemx_target(cubemx_nucleo_f401re "${PREBENCH_CUBEMX_TEST_PROJECT}")
target_compile_definitions(cubemx_nucleo_f401re PRIVATE main=prebench_firmware_main)
add_executable(test_cubemx test_cubemx.c)
target_link_libraries(test_cubemx PRIVATE cubemx_nucleo_f401re vmcu_fake_app prebench_stm32_hal vmcu)
target_compile_options(test_cubemx PRIVATE ${VMCU_WARNING_FLAGS})
add_test(NAME test_cubemx COMMAND test_cubemx)

# Modèle templates/blinky-stm32-hal, même principe.
set(PREBENCH_BLINKY_TEMPLATE "${PROJECT_SOURCE_DIR}/../templates/blinky-stm32-hal")
prebench_cubemx_sources(_blinky_srcs "${PREBENCH_BLINKY_TEMPLATE}")
add_library(template_blinky OBJECT ${_blinky_srcs})
prebench_configure_cubemx_target(template_blinky "${PREBENCH_BLINKY_TEMPLATE}")
target_compile_definitions(template_blinky PRIVATE main=prebench_firmware_main)
add_executable(test_blinky test_blinky.c)
target_link_libraries(test_blinky PRIVATE template_blinky vmcu_fake_app prebench_stm32_hal vmcu)
target_compile_options(test_blinky PRIVATE ${VMCU_WARNING_FLAGS})
add_test(NAME test_blinky COMMAND test_blinky)

set_tests_properties(test_wire test_client test_stm32_hal test_stm32_hal_stubs test_cubemx test_blinky
                     PROPERTIES TIMEOUT 30)
