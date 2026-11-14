# tests/pwm-20khz/CMakeLists.txt — Test de charge PWM (phase 2, étape 8), cible « sim ».
#   Ouvrir le dossier dans Prebench, puis Démarrer.

cmake_minimum_required(VERSION 3.20)
project(prebench_pwm_20khz LANGUAGES C)

set(PREBENCH_ROOT "${CMAKE_CURRENT_SOURCE_DIR}/../.." CACHE PATH "Racine de Prebench (contient libvmcu/)")
set(VMCU_BOARD "nucleo-f401re" CACHE STRING "" FORCE)
set(VMCU_FRAMEWORK "stm32-hal" CACHE STRING "" FORCE)
add_subdirectory("${PREBENCH_ROOT}/libvmcu" libvmcu)

add_executable(firmware main.c)
target_compile_definitions(firmware PRIVATE USE_HAL_DRIVER STM32F401xE)
target_link_libraries(firmware PRIVATE prebench_stm32_hal)
target_compile_options(firmware PRIVATE ${VMCU_WARNING_FLAGS})
