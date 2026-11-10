# libvmcu/cmake/PrebenchFirmware.cmake — Cible « sim » d'un projet STM32CubeMX non modifié.
#
# Inclus par libvmcu/CMakeLists.txt. Compile les sources du projet pour
# l'ordinateur et les lie au shim STM32 HAL :
#   - sources : <projet>/Core/Src/*.c ;
#   - exclues, car propres à la vraie puce : system_stm32*.c (registres CMSIS),
#     syscalls.c et sysmem.c (newlib, symboles de l'éditeur de liens), ainsi
#     que le fichier de démarrage .s et le dossier Drivers/ (remplacé par le shim) ;
#   - en-têtes : <projet>/Core/Inc, puis ceux du shim. Les en-têtes de
#     Drivers/ ne sont jamais ajoutés : stm32f4xx_hal.h est celui du shim.
#
# Les avertissements du code utilisateur sont affichés, jamais bloquants.

set(PREBENCH_SIM_EXCLUDE_REGEX "/(system_stm32[a-z0-9]*|syscalls|sysmem)\\.c$"
    CACHE STRING "Sources CubeMX exclues de la cible « sim »")

# prebench_cubemx_sources(<variable> <dossier projet>)
#   Liste les sources C d'un projet CubeMX à compiler pour la cible « sim ».
function(prebench_cubemx_sources out_var project_dir)
  file(GLOB _srcs CONFIGURE_DEPENDS "${project_dir}/Core/Src/*.c")
  list(FILTER _srcs EXCLUDE REGEX "${PREBENCH_SIM_EXCLUDE_REGEX}")
  if(NOT _srcs)
    message(FATAL_ERROR "Aucune source dans ${project_dir}/Core/Src : est-ce un projet STM32CubeMX ?")
  endif()
  set(${out_var} "${_srcs}" PARENT_SCOPE)
endfunction()

# prebench_configure_cubemx_target(<cible> <dossier projet>)
#   Ajoute en-têtes, définitions et shim à une cible déjà créée.
function(prebench_configure_cubemx_target target project_dir)
  target_include_directories(${target} PRIVATE "${project_dir}/Core/Inc")
  target_compile_definitions(${target} PRIVATE USE_HAL_DRIVER STM32F401xE)
  target_compile_options(${target} PRIVATE -Wall)
  target_link_libraries(${target} PRIVATE prebench_stm32_hal)
endfunction()

# prebench_add_cubemx_firmware(<cible> <dossier projet>)
#   Exécutable « sim » d'un projet CubeMX, lancé ensuite par Prebench.
function(prebench_add_cubemx_firmware target project_dir)
  prebench_cubemx_sources(_srcs "${project_dir}")
  add_executable(${target} ${_srcs})
  prebench_configure_cubemx_target(${target} "${project_dir}")
endfunction()
