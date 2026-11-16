/*
 * tests/i2c-scan/main.c — Bus I2C du HAL STM32 (phase 3, étape 2).
 *
 * Écrit comme un projet STM32CubeMX : MX_I2C1_Init, HAL_I2C_MspInit (PB8 SCL,
 * PB9 SDA en AF4 drain ouvert), IRQ I2C1_EV activée dans le NVIC.
 * 1. Balayage du bus : HAL_I2C_IsDeviceReady de 0x08 à 0x77.
 * 2. Mem_Read bloquant du registre 0x00 du joker (0x2A dans prebench.toml).
 * 3. Mem_Write de 0xA5 dans 0x10, relu par Mem_Read.
 * 4. Adresse absente (0x51) : NACK, ErrorCode = HAL_I2C_ERROR_AF.
 * 5. Toutes les 200 ms : HAL_I2C_Mem_Read_IT du registre 0x20 (curseur du
 *    joker) ; HAL_I2C_MemRxCpltCallback lève un drapeau, la boucle principale
 *    envoie « curseur:<valeur> » au traceur.
 */
#include <stdio.h>
#include <string.h>

#include "stm32f4xx_hal.h"

#define JOKER_ADDR (0x50u << 1) /* adresse 7 bits décalée, comme dans le HAL */

UART_HandleTypeDef huart2;
I2C_HandleTypeDef hi2c1;

static uint8_t slider;
static volatile int slider_ready;
static volatile int i2c_error;

/* Ligne de texte sur USART2 (moniteur série et traceur). */
#define PRINT(...)                                                                  \
    do {                                                                            \
        char line_[64];                                                             \
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
        HAL_NVIC_SetPriority(I2C1_EV_IRQn, 0, 0);
        HAL_NVIC_EnableIRQ(I2C1_EV_IRQn);
    }
}

void HAL_I2C_MemRxCpltCallback(I2C_HandleTypeDef *hi2c)
{
    if (hi2c == &hi2c1) {
        slider_ready = 1;
    }
}

void HAL_I2C_ErrorCallback(I2C_HandleTypeDef *hi2c)
{
    if (hi2c == &hi2c1) {
        i2c_error = 1;
    }
}

static void MX_USART2_UART_Init(void)
{
    __HAL_RCC_USART2_CLK_ENABLE();
    huart2.Instance = USART2;
    huart2.Init.BaudRate = 115200;
    huart2.Init.WordLength = UART_WORDLENGTH_8B;
    huart2.Init.StopBits = UART_STOPBITS_1;
    huart2.Init.Parity = UART_PARITY_NONE;
    huart2.Init.Mode = UART_MODE_TX_RX;
    HAL_UART_Init(&huart2);
}

static void MX_I2C1_Init(void)
{
    hi2c1.Instance = I2C1;
    hi2c1.Init.ClockSpeed = 100000;
    hi2c1.Init.DutyCycle = I2C_DUTYCYCLE_2;
    hi2c1.Init.OwnAddress1 = 0;
    hi2c1.Init.AddressingMode = I2C_ADDRESSINGMODE_7BIT;
    hi2c1.Init.DualAddressMode = I2C_DUALADDRESS_DISABLE;
    hi2c1.Init.OwnAddress2 = 0;
    hi2c1.Init.GeneralCallMode = I2C_GENERALCALL_DISABLE;
    hi2c1.Init.NoStretchMode = I2C_NOSTRETCH_DISABLE;
    HAL_I2C_Init(&hi2c1);
}

int main(void)
{
    HAL_Init();
    MX_USART2_UART_Init();
    MX_I2C1_Init();

    /* 1. Balayage */
    unsigned found = 0;
    for (unsigned a = 0x08; a <= 0x77; ++a) {
        if (HAL_I2C_IsDeviceReady(&hi2c1, (uint16_t)(a << 1), 1, 10) == HAL_OK) {
            PRINT("I2C trouvé : 0x%02X\r\n", a);
            found++;
        }
    }
    PRINT("Balayage fini : %u composant(s)\r\n", found);

    /* 2. Lecture d'un registre fixe */
    uint8_t v = 0;
    if (HAL_I2C_Mem_Read(&hi2c1, JOKER_ADDR, 0x00, I2C_MEMADD_SIZE_8BIT, &v, 1, 100) == HAL_OK) {
        PRINT("reg 0x00 = 0x%02X\r\n", v);
    }

    /* 3. Écriture puis relecture */
    uint8_t w = 0xA5;
    HAL_I2C_Mem_Write(&hi2c1, JOKER_ADDR, 0x10, I2C_MEMADD_SIZE_8BIT, &w, 1, 100);
    v = 0;
    HAL_I2C_Mem_Read(&hi2c1, JOKER_ADDR, 0x10, I2C_MEMADD_SIZE_8BIT, &v, 1, 100);
    PRINT("reg 0x10 = 0x%02X\r\n", v);

    /* 4. Adresse sans composant */
    HAL_StatusTypeDef st = HAL_I2C_Mem_Read(&hi2c1, 0x51u << 1, 0x00, I2C_MEMADD_SIZE_8BIT, &v, 1, 100);
    PRINT("0x51 : %s\r\n", (st == HAL_ERROR && HAL_I2C_GetError(&hi2c1) == HAL_I2C_ERROR_AF) ? "NACK (AF)" : "inattendu");

    /* 5. Lecture par interruption, 5 fois par seconde */
    uint32_t next = HAL_GetTick();
    while (1) {
        if ((int32_t)(HAL_GetTick() - next) >= 0 && HAL_I2C_GetState(&hi2c1) == HAL_I2C_STATE_READY) {
            next += 200;
            (void)HAL_I2C_Mem_Read_IT(&hi2c1, JOKER_ADDR, 0x20, I2C_MEMADD_SIZE_8BIT, &slider, 1);
        }
        if (slider_ready) {
            slider_ready = 0;
            PRINT("curseur:%u\r\n", slider);
        }
        if (i2c_error) {
            i2c_error = 0;
            PRINT("erreur I2C 0x%02X\r\n", (unsigned)HAL_I2C_GetError(&hi2c1));
        }
        HAL_Delay(1);
    }
}
