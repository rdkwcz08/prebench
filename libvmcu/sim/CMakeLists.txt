# libvmcu/sim/CMakeLists.txt — Build « sim » d'un projet STM32CubeMX, sans le modifier.
#
#   cmake -S libvmcu/sim -B build-sim -DPREBENCH_PROJECT=<dossier du projet CubeMX>
#   cmake --build build-sim
#   -> exécutable build-sim/firmware (firmware.exe sous Windows), lancé par Prebench.
#
# Windows : ajouter -G "MinGW Makefiles" (gcc de MinGW-w64, section 12.3).

cmake_minimum_required(VERSION 3.20)
project(prebench_sim LANGUAGES C)

set(PREBENCH_PROJECT "" CACHE PATH "Dossier du projet STM32CubeMX à simuler")
if(NOT PREBENCH_PROJECT)
  message(FATAL_ERROR "Préciser le projet : -DPREBENCH_PROJECT=<dossier du projet CubeMX>")
endif()
get_filename_component(_project "${PREBENCH_PROJECT}" ABSOLUTE BASE_DIR "${CMAKE_SOURCE_DIR}/../..")

set(VMCU_BUILD_TESTS OFF CACHE BOOL "" FORCE)
add_subdirectory(.. libvmcu)

prebench_add_cubemx_firmware(firmware "${_project}")
