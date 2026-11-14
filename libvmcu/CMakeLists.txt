# libvmcu/CMakeLists.txt — Bibliothèque libvmcu et shims.
#
# Utilisation dans un projet firmware (cible « sim ») :
#   set(VMCU_BOARD "nucleo-f401re")
#   set(VMCU_FRAMEWORK "stm32-hal")
#   add_subdirectory(<prebench>/libvmcu libvmcu)
#   target_link_libraries(firmware PRIVATE prebench_stm32_hal)
# Projet STM32CubeMX non modifié : libvmcu/sim/CMakeLists.txt
# (fonctions de cmake/PrebenchFirmware.cmake).
#
# Tests (depuis la racine du dépôt) :
#   cmake -S libvmcu -B build-libvmcu -DVMCU_WERROR=ON
#   cmake --build build-libvmcu --parallel
#   ctest --test-dir build-libvmcu --output-on-failure
#
# Compilateurs : clang (macOS, Linux), gcc MinGW-w64 (Windows). MSVC n'est pas
# pris en charge (section 12.3).

cmake_minimum_required(VERSION 3.20)
project(libvmcu VERSION 0.1.0 LANGUAGES C)

if(MSVC)
  message(FATAL_ERROR "libvmcu se compile avec clang ou MinGW-w64, pas MSVC (cahier des charges, 12.3).")
endif()

set(VMCU_BOARD "nucleo-f401re" CACHE STRING "Carte simulée (boards/<carte>.toml)")
set(VMCU_FRAMEWORK "stm32-hal" CACHE STRING "Framework du firmware (stm32-hal, esp-idf, arduino)")
option(VMCU_WERROR "Avertissements traités comme des erreurs" OFF)
option(VMCU_BUILD_TESTS "Construire les tests de libvmcu" ${PROJECT_IS_TOP_LEVEL})

set(VMCU_WARNING_FLAGS -Wall -Wextra -Wpedantic -Wshadow -Wstrict-prototypes)
if(VMCU_WERROR)
  list(APPEND VMCU_WARNING_FLAGS -Werror)
endif()

find_package(Threads REQUIRED)

if(WIN32)
  set(_vmcu_platform platform/vmcu_platform_win32.c)
else()
  set(_vmcu_platform platform/vmcu_platform_posix.c)
endif()

add_library(vmcu STATIC
  core/vmcu_wire.c
  core/vmcu_client.c
  core/vmcu_time.c
  core/vmcu_gpio.c
  core/vmcu_uart.c
  core/vmcu_pwm.c
  core/vmcu_adc.c
  core/vmcu_diag.c
  core/vmcu_unsupported.c
  ${_vmcu_platform}
)
target_include_directories(vmcu
  PUBLIC  include
  PRIVATE core platform
)
target_compile_features(vmcu PUBLIC c_std_11)
set_target_properties(vmcu PROPERTIES C_EXTENSIONS OFF)
target_compile_definitions(vmcu PRIVATE
  VMCU_DEFAULT_BOARD="${VMCU_BOARD}"
  VMCU_DEFAULT_FRAMEWORK="${VMCU_FRAMEWORK}"
)
target_compile_options(vmcu PRIVATE ${VMCU_WARNING_FLAGS})
target_link_libraries(vmcu PUBLIC Threads::Threads)

add_subdirectory(shims/stm32-hal)
include(cmake/PrebenchFirmware.cmake)

if(VMCU_BUILD_TESTS)
  enable_testing()
  add_subdirectory(tests)
endif()
