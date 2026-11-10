# templates/blinky-stm32-hal/CMakeLists.txt — Cible « sim » du blinky STM32 HAL.
#
# Compile le projet pour l'ordinateur, lié à libvmcu et au shim STM32 HAL :
#   cmake -S templates/blinky-stm32-hal -B build-blinky
#   cmake --build build-blinky
#   -> build-blinky/firmware (firmware.exe sous Windows), lancé par Prebench.
#
# macOS : clang (outils en ligne de commande Xcode).
# Windows : gcc de MinGW-w64, avec -G "MinGW Makefiles" (MSVC refusé, section 12.3).
#
# PREBENCH_ROOT : dossier qui contient libvmcu/. Par défaut, la racine du dépôt
# Prebench ; un projet créé depuis ce modèle reçoit le chemin installé par l'app.

cmake_minimum_required(VERSION 3.20)
project(blinky_stm32_hal LANGUAGES C)

set(PREBENCH_ROOT "${CMAKE_CURRENT_SOURCE_DIR}/../.." CACHE PATH "Racine de Prebench (contient libvmcu/)")
set(VMCU_BOARD "nucleo-f401re" CACHE STRING "" FORCE)
set(VMCU_FRAMEWORK "stm32-hal" CACHE STRING "" FORCE)
add_subdirectory("${PREBENCH_ROOT}/libvmcu" libvmcu)

prebench_add_cubemx_firmware(firmware "${CMAKE_CURRENT_SOURCE_DIR}")
