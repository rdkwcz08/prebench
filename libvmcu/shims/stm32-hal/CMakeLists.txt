# libvmcu/shims/stm32-hal/CMakeLists.txt — Shim STM32 HAL (bibliothèque statique).
#
# Les fonctions « __weak » du HAL (callbacks, MspInit, gestionnaires EXTI) sont
# chacune dans leur propre fichier objet : l'éditeur de liens ne les prend dans
# la bibliothèque que si le projet ne les définit pas. Même comportement avec
# ld64 (macOS) et ld de MinGW, sans __attribute__((weak)).

add_library(prebench_stm32_hal STATIC
  src/stm32_hal.c
  src/stm32_hal_gpio.c
  src/stm32_hal_periph.c
  src/stm32_hal_tim.c
  src/default_exti_callback.c
  src/default_i2c_addr_callback.c
  src/default_msp_init.c
  src/default_msp_deinit.c
)

# Un gestionnaire EXTI par défaut par IRQ : « nom:première ligne:dernière ligne ».
set(_prebench_exti_handlers
  "EXTI0_IRQHandler:0:0"
  "EXTI1_IRQHandler:1:1"
  "EXTI2_IRQHandler:2:2"
  "EXTI3_IRQHandler:3:3"
  "EXTI4_IRQHandler:4:4"
  "EXTI9_5_IRQHandler:5:9"
  "EXTI15_10_IRQHandler:10:15"
)
foreach(_item IN LISTS _prebench_exti_handlers)
  string(REPLACE ":" ";" _entry "${_item}")
  list(GET _entry 0 HANDLER)
  list(GET _entry 1 FIRST)
  list(GET _entry 2 LAST)
  set(_out "${CMAKE_CURRENT_BINARY_DIR}/generated/default_${HANDLER}.c")
  configure_file(src/default_exti_handler.c.in "${_out}" @ONLY)
  target_sources(prebench_stm32_hal PRIVATE "${_out}")
endforeach()

# Fonctions __weak des modules périphériques (callbacks, MspInit), une par
# fichier généré : « nom:type du handle:paramètres en plus:utilisation ».
set(_prebench_weak_defaults
  "HAL_UART_MspInit:UART_HandleTypeDef::"
  "HAL_UART_MspDeInit:UART_HandleTypeDef::"
  "HAL_UART_TxCpltCallback:UART_HandleTypeDef::"
  "HAL_UART_TxHalfCpltCallback:UART_HandleTypeDef::"
  "HAL_UART_RxCpltCallback:UART_HandleTypeDef::"
  "HAL_UART_RxHalfCpltCallback:UART_HandleTypeDef::"
  "HAL_UART_ErrorCallback:UART_HandleTypeDef::"
  "HAL_UART_AbortCpltCallback:UART_HandleTypeDef::"
  "HAL_UART_AbortTransmitCpltCallback:UART_HandleTypeDef::"
  "HAL_UART_AbortReceiveCpltCallback:UART_HandleTypeDef::"
  "HAL_TIM_Base_MspInit:TIM_HandleTypeDef::"
  "HAL_TIM_Base_MspDeInit:TIM_HandleTypeDef::"
  "HAL_TIM_OC_MspInit:TIM_HandleTypeDef::"
  "HAL_TIM_OC_MspDeInit:TIM_HandleTypeDef::"
  "HAL_TIM_PWM_MspInit:TIM_HandleTypeDef::"
  "HAL_TIM_PWM_MspDeInit:TIM_HandleTypeDef::"
  "HAL_TIM_IC_MspInit:TIM_HandleTypeDef::"
  "HAL_TIM_IC_MspDeInit:TIM_HandleTypeDef::"
  "HAL_TIM_Encoder_MspInit:TIM_HandleTypeDef::"
  "HAL_TIM_Encoder_MspDeInit:TIM_HandleTypeDef::"
  "HAL_TIM_PeriodElapsedCallback:TIM_HandleTypeDef::"
  "HAL_TIM_PeriodElapsedHalfCpltCallback:TIM_HandleTypeDef::"
  "HAL_TIM_OC_DelayElapsedCallback:TIM_HandleTypeDef::"
  "HAL_TIM_IC_CaptureCallback:TIM_HandleTypeDef::"
  "HAL_TIM_IC_CaptureHalfCpltCallback:TIM_HandleTypeDef::"
  "HAL_TIM_PWM_PulseFinishedCallback:TIM_HandleTypeDef::"
  "HAL_TIM_PWM_PulseFinishedHalfCpltCallback:TIM_HandleTypeDef::"
  "HAL_TIM_TriggerCallback:TIM_HandleTypeDef::"
  "HAL_TIM_TriggerHalfCpltCallback:TIM_HandleTypeDef::"
  "HAL_TIM_ErrorCallback:TIM_HandleTypeDef::"
  "HAL_TIMEx_CommutCallback:TIM_HandleTypeDef::"
  "HAL_TIMEx_BreakCallback:TIM_HandleTypeDef::"
  "HAL_ADC_MspInit:ADC_HandleTypeDef::"
  "HAL_ADC_MspDeInit:ADC_HandleTypeDef::"
  "HAL_ADC_ConvCpltCallback:ADC_HandleTypeDef::"
  "HAL_ADC_ConvHalfCpltCallback:ADC_HandleTypeDef::"
  "HAL_ADC_LevelOutOfWindowCallback:ADC_HandleTypeDef::"
  "HAL_ADC_ErrorCallback:ADC_HandleTypeDef::"
  "HAL_ADCEx_InjectedConvCpltCallback:ADC_HandleTypeDef::"
  "HAL_I2C_MspInit:I2C_HandleTypeDef::"
  "HAL_I2C_MspDeInit:I2C_HandleTypeDef::"
  "HAL_I2C_MasterTxCpltCallback:I2C_HandleTypeDef::"
  "HAL_I2C_MasterRxCpltCallback:I2C_HandleTypeDef::"
  "HAL_I2C_SlaveTxCpltCallback:I2C_HandleTypeDef::"
  "HAL_I2C_SlaveRxCpltCallback:I2C_HandleTypeDef::"
  "HAL_I2C_ListenCpltCallback:I2C_HandleTypeDef::"
  "HAL_I2C_MemTxCpltCallback:I2C_HandleTypeDef::"
  "HAL_I2C_MemRxCpltCallback:I2C_HandleTypeDef::"
  "HAL_I2C_ErrorCallback:I2C_HandleTypeDef::"
  "HAL_I2C_AbortCpltCallback:I2C_HandleTypeDef::"
  "HAL_SPI_MspInit:SPI_HandleTypeDef::"
  "HAL_SPI_MspDeInit:SPI_HandleTypeDef::"
  "HAL_SPI_TxCpltCallback:SPI_HandleTypeDef::"
  "HAL_SPI_RxCpltCallback:SPI_HandleTypeDef::"
  "HAL_SPI_TxRxCpltCallback:SPI_HandleTypeDef::"
  "HAL_SPI_TxHalfCpltCallback:SPI_HandleTypeDef::"
  "HAL_SPI_RxHalfCpltCallback:SPI_HandleTypeDef::"
  "HAL_SPI_TxRxHalfCpltCallback:SPI_HandleTypeDef::"
  "HAL_SPI_ErrorCallback:SPI_HandleTypeDef::"
  "HAL_SPI_AbortCpltCallback:SPI_HandleTypeDef::"
  "HAL_UARTEx_RxEventCallback:UART_HandleTypeDef:, uint16_t Size:UNUSED(Size)"
)
foreach(_item IN LISTS _prebench_weak_defaults)
  string(REPLACE ":" ";" _entry "${_item}")
  list(GET _entry 0 NAME)
  list(GET _entry 1 TYPE)
  list(GET _entry 2 EXTRA_DECL)
  list(GET _entry 3 EXTRA_USE)
  if(EXTRA_USE STREQUAL "")
    set(EXTRA_USE "(void)0")
  endif()
  set(_out "${CMAKE_CURRENT_BINARY_DIR}/generated/default_${NAME}.c")
  configure_file(src/default_hal_callback.c.in "${_out}" @ONLY)
  target_sources(prebench_stm32_hal PRIVATE "${_out}")
endforeach()

target_include_directories(prebench_stm32_hal
  PUBLIC  include
  PRIVATE src
)
target_link_libraries(prebench_stm32_hal PUBLIC vmcu)
target_compile_options(prebench_stm32_hal PRIVATE ${VMCU_WARNING_FLAGS})
