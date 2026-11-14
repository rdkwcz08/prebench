# tests/pwm-fade/CMakeLists.txt — Exemple PWM (phase 2, étape 8) : fondu de LD2 par TIM2_CH1 sur PA5.
#   Ouvrir le dossier dans Prebench, puis Démarrer. Structure STM32CubeMX (Core/Inc, Core/Src).

cmake_minimum_required(VERSION 3.20)
project(prebench_pwm_fade LANGUAGES C)

set(PREBENCH_ROOT "${CMAKE_CURRENT_SOURCE_DIR}/../.." CACHE PATH "Racine de Prebench (contient libvmcu/)")
set(VMCU_BOARD "nucleo-f401re" CACHE STRING "" FORCE)
set(VMCU_FRAMEWORK "stm32-hal" CACHE STRING "" FORCE)
add_subdirectory("${PREBENCH_ROOT}/libvmcu" libvmcu)

prebench_add_cubemx_firmware(firmware "${CMAKE_CURRENT_SOURCE_DIR}")
