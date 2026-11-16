/*
 * tests/mpu-tilt/main.c — Centrale inertielle MPU6050 en I2C (phase 3, étape 5).
 *
 * I2C1 (PB8 SCL, PB9 SDA, AF4), MPU6050 en 0x68, comme dans les exemples
 * courants : réveil (PWR_MGMT_1 = 0), plage ±2 g et ±250 °/s, puis toutes les
 * 50 ms une lecture groupée des 14 octets de mesure depuis 0x3B.
 * USART2 affiche « WHO_AM_I = 0x68 », puis au format du traceur :
 * « ax:<mg>,ay:<mg>,az:<mg>,gx:<°/s>,gy:<°/s>,temp:<°C> ».
 * Réglez l'inclinaison dans l'onglet Environnement : les courbes suivent.
 */
#include <stdio.h>

#include "stm32f4xx_hal.h"

#define MPU_ADDR (0x68u << 1)

UART_HandleTypeDef huart2;
I2C_HandleTypeDef hi2c1;

#define PRINT(...)                                                                  \
    do {                                                                            \
        char line_[80];                                                             \
        int len_ = snprintf(line_, sizeof line_, __VA_ARGS__);                      \
        HAL_UART_Transmit(&huart2, (const uint8_t *)line_, (uint16_t)len_, 100);    \
    } while (0)

void HAL_I2C_MspInit(I2C_HandleTypeDef *hi2c)
{
    if (hi2c->Instance == I2C1) {
        GPIO_InitTypeDef gpio = { 0 };
        __HAL_RCC_GPIOB_CLK_ENABLE();
        gpio.Pin = GPIO_PIN_8 | GPIO_PIN_9;
        gpio.Mode = GPIO_MODE_AF_OD;
        gpio.Pull = GPIO_NOPULL;
        gpio.Speed = GPIO_SPEED_FREQ_VERY_HIGH;
        gpio.Alternate = GPIO_AF4_I2C1;
        HAL_GPIO_Init(GPIOB, &gpio);
        __HAL_RCC_I2C1_CLK_ENABLE();
    }
}

static int16_t be16(const uint8_t *p)
{
    return (int16_t)(((uint16_t)p[0] << 8) | p[1]);
}

int main(void)
{
    HAL_Init();
    huart2.Instance = USART2;
    huart2.Init.BaudRate = 115200;
    huart2.Init.Mode = UART_MODE_TX_RX;
    HAL_UART_Init(&huart2);
    hi2c1.Instance = I2C1;
    hi2c1.Init.ClockSpeed = 400000;
    hi2c1.Init.AddressingMode = I2C_ADDRESSINGMODE_7BIT;
    HAL_I2C_Init(&hi2c1);

    while (HAL_I2C_IsDeviceReady(&hi2c1, MPU_ADDR, 1, 10) != HAL_OK) {
        HAL_Delay(100); /* module pas encore alimenté */
    }
    uint8_t who = 0;
    HAL_I2C_Mem_Read(&hi2c1, MPU_ADDR, 0x75, I2C_MEMADD_SIZE_8BIT, &who, 1, 100);
    PRINT("WHO_AM_I = 0x%02X\r\n", who);

    uint8_t v = 0x00; /* PWR_MGMT_1 : réveil */
    HAL_I2C_Mem_Write(&hi2c1, MPU_ADDR, 0x6B, I2C_MEMADD_SIZE_8BIT, &v, 1, 100);
    v = 0x00; /* GYRO_CONFIG : ±250 °/s */
    HAL_I2C_Mem_Write(&hi2c1, MPU_ADDR, 0x1B, I2C_MEMADD_SIZE_8BIT, &v, 1, 100);
    v = 0x00; /* ACCEL_CONFIG : ±2 g */
    HAL_I2C_Mem_Write(&hi2c1, MPU_ADDR, 0x1C, I2C_MEMADD_SIZE_8BIT, &v, 1, 100);

    while (1) {
        uint8_t d[14];
        if (HAL_I2C_Mem_Read(&hi2c1, MPU_ADDR, 0x3B, I2C_MEMADD_SIZE_8BIT, d, sizeof d, 100) == HAL_OK) {
            long ax = (long)be16(&d[0]) * 1000L / 16384L;
            long ay = (long)be16(&d[2]) * 1000L / 16384L;
            long az = (long)be16(&d[4]) * 1000L / 16384L;
            long temp10 = (long)be16(&d[6]) * 10L / 340L + 365L; /* dixièmes de °C */
            long gx = (long)be16(&d[8]) / 131L;
            long gy = (long)be16(&d[10]) / 131L;
            PRINT("ax:%ld,ay:%ld,az:%ld,gx:%ld,gy:%ld,temp:%ld.%ld\r\n", ax, ay, az, gx, gy,
                  temp10 / 10L, (temp10 < 0 ? -temp10 : temp10) % 10L);
        }
        HAL_Delay(50);
    }
}
