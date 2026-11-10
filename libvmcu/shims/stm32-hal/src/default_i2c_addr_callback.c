/*
 * libvmcu/shims/stm32-hal/src/default_i2c_addr_callback.c — HAL_I2C_AddrCallback par défaut
 * (voir default_exti_callback.c).
 */
#include "stm32f4xx_hal.h"

void HAL_I2C_AddrCallback(I2C_HandleTypeDef *hi2c, uint8_t TransferDirection, uint16_t AddrMatchCode)
{
    UNUSED(hi2c);
    UNUSED(TransferDirection);
    UNUSED(AddrMatchCode);
}
