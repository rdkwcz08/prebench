# tests/latency/CMakeLists.txt — Outil de mesure de latence (cible « sim » uniquement).
#   cmake -S tests/latency -B build-latency && cmake --build build-latency
# Lancé depuis Prebench (projet tests/latency, composant « echo » placé).

cmake_minimum_required(VERSION 3.20)
project(prebench_latency LANGUAGES C)

set(PREBENCH_ROOT "${CMAKE_CURRENT_SOURCE_DIR}/../.." CACHE PATH "Racine de Prebench (contient libvmcu/)")
set(VMCU_BOARD "nucleo-f401re" CACHE STRING "" FORCE)
set(VMCU_FRAMEWORK "stm32-hal" CACHE STRING "" FORCE)
add_subdirectory("${PREBENCH_ROOT}/libvmcu" libvmcu)

add_executable(firmware main.c)
target_link_libraries(firmware PRIVATE vmcu)
target_compile_options(firmware PRIVATE ${VMCU_WARNING_FLAGS})
