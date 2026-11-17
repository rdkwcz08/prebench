/*
 * tests/projet-type/main.c — Projet réaliste de mesure du budget (section 11.1).
 *
 * Un projet comme on en écrit vraiment, plutôt qu'une paillasse qui anime
 * tout en permanence (tests/budget, test de stress) :
 * - LD2 (PA5) clignote à 1 Hz ; B1 (PC13) en interruption allume LD2 tant
 *   qu'il est appuyé ;
 * - MPU6050 en 0x68 lu toutes les 50 ms (I2C1, PB8 SCL, PB9 SDA) ;
 * - écran OLED SSD1306 en 0x3C : accélération sur trois axes, en mg, toutes
 *   les 100 ms (seuls les pixels qui changent partent vers l'interface) ;
 * - USART2 : une ligne pour le traceur toutes les 200 ms.
 * L'inclinaison se règle dans l'onglet Environnement.
 */
#include <stdio.h>
#include <string.h>

#include "stm32f4xx_hal.h"

#define OLED_ADDR (0x3Cu << 1)
#define MPU_ADDR  (0x68u << 1)

UART_HandleTypeDef huart2;
I2C_HandleTypeDef hi2c1;

static uint8_t frame[128 * 8];
static volatile int pressed;

/* Police 5 × 7 (colonnes, bit 0 en haut), dessinée pour les exemples. */
static const struct {
    char ch;
    uint8_t cols[5];
} FONT[] = {
    { ' ', { 0x00, 0x00, 0x00, 0x00, 0x00 } },
    { '.', { 0x60, 0x60, 0x60, 0x00, 0x00 } },
    { '0', { 0x3E, 0x51, 0x49, 0x45, 0x3E } },
    { '1', { 0x00, 0x42, 0x7F, 0x40, 0x00 } },
    { '2', { 0x42, 0x61, 0x51, 0x49, 0x46 } },
    { '3', { 0x21, 0x41, 0x45, 0x4B, 0x31 } },
    { '4', { 0x18, 0x14, 0x12, 0x7F, 0x10 } },
    { '5', { 0x27, 0x45, 0x45, 0x45, 0x39 } },
    { '6', { 0x3C, 0x4A, 0x49, 0x49, 0x30 } },
    { '7', { 0x01, 0x71, 0x09, 0x05, 0x03 } },
    { '8', { 0x36, 0x49, 0x49, 0x49, 0x36 } },
    { '9', { 0x06, 0x49, 0x49, 0x29, 0x1E } },
    { ':', { 0x36, 0x36, 0x36, 0x00, 0x00 } },
    { '=', { 0x14, 0x14, 0x14, 0x14, 0x14 } },
    { 'P', { 0x7F, 0x09, 0x09, 0x09, 0x06 } },
    { 'b', { 0x7F, 0x48, 0x44, 0x44, 0x38 } },
    { 'c', { 0x38, 0x44, 0x44, 0x44, 0x20 } },
    { 'e', { 0x38, 0x54, 0x54, 0x54, 0x18 } },
    { 'h', { 0x7F, 0x08, 0x04, 0x04, 0x78 } },
    { 'n', { 0x7C, 0x08, 0x04, 0x04, 0x78 } },
    { 'r', { 0x7C, 0x08, 0x04, 0x04, 0x08 } },
    { 's', { 0x48, 0x54, 0x54, 0x54, 0x24 } },
    { 't', { 0x04, 0x3F, 0x44, 0x40, 0x20 } },
    { '-', { 0x08, 0x08, 0x08, 0x08, 0x08 } },
    { 'x', { 0x44, 0x28, 0x10, 0x28, 0x44 } },
    { 'y', { 0x0C, 0x50, 0x50, 0x50, 0x3C } },
    { 'z', { 0x44, 0x64, 0x54, 0x4C, 0x44 } },
    { 'm', { 0x7C, 0x04, 0x18, 0x04, 0x78 } },
    { 'g', { 0x08, 0x54, 0x54, 0x54, 0x3C } },
};

#define PRINT(...)                                                                  \
    do {                                                                            \
        char line_[64];                                                             \
        int len_ = snprintf(line_, sizeof line_, __VA_ARGS__);                      \
        HAL_UART_Transmit(&huart2, (const uint8_t *)line_, (uint16_t)len_, 100);    \
    } while (0)

static void draw_text(unsigned x, unsigned page, const char *s)
{
    for (; *s != '\0' && x + 5 <= 128; ++s, x += 6) {
        for (size_t i = 0; i < sizeof FONT / sizeof FONT[0]; ++i) {
            if (FONT[i].ch == *s) {
                memcpy(&frame[page * 128 + x], FONT[i].cols, 5);
                break;
            }
        }
    }
}

static void oled_cmds(const uint8_t *cmds, uint16_t n)
{
    HAL_I2C_Mem_Write(&hi2c1, OLED_ADDR, 0x00, I2C_MEMADD_SIZE_8BIT, (uint8_t *)cmds, n, 100);
}

static void oled_show(void)
{
    static const uint8_t window[] = { 0x21, 0, 127, 0x22, 0, 7 };
    oled_cmds(window, sizeof window);
    HAL_I2C_Mem_Write(&hi2c1, OLED_ADDR, 0x40, I2C_MEMADD_SIZE_8BIT, frame, sizeof frame, 100);
}

static void reg_write(uint8_t reg, uint8_t value)
{
    HAL_I2C_Mem_Write(&hi2c1, MPU_ADDR, reg, I2C_MEMADD_SIZE_8BIT, &value, 1, 100);
}

void HAL_I2C_MspInit(I2C_HandleTypeDef *hi2c)
{
    if (hi2c->Instance == I2C1) {
        GPIO_InitTypeDef gpio = { 0 };
        __HAL_RCC_GPIOB_CLK_ENABLE();
        gpio.Pin = GPIO_PIN_8 | GPIO_PIN_9;
        gpio.Mode = GPIO_MODE_AF_OD;
        gpio.Alternate = GPIO_AF4_I2C1;
        HAL_GPIO_Init(GPIOB, &gpio);
        __HAL_RCC_I2C1_CLK_ENABLE();
    }
}

void HAL_GPIO_EXTI_Callback(uint16_t pin)
{
    if (pin == GPIO_PIN_13) {
        pressed = HAL_GPIO_ReadPin(GPIOC, GPIO_PIN_13) == GPIO_PIN_RESET;
    }
}

void EXTI15_10_IRQHandler(void)
{
    HAL_GPIO_EXTI_IRQHandler(GPIO_PIN_13);
}

int main(void)
{
    HAL_Init();
    __HAL_RCC_GPIOA_CLK_ENABLE();
    __HAL_RCC_GPIOC_CLK_ENABLE();
    GPIO_InitTypeDef gpio = { 0 };
    gpio.Pin = GPIO_PIN_5;
    gpio.Mode = GPIO_MODE_OUTPUT_PP;
    HAL_GPIO_Init(GPIOA, &gpio);
    gpio.Pin = GPIO_PIN_13;
    gpio.Mode = GPIO_MODE_IT_RISING_FALLING;
    HAL_GPIO_Init(GPIOC, &gpio);
    HAL_NVIC_EnableIRQ(EXTI15_10_IRQn);

    huart2.Instance = USART2;
    huart2.Init.BaudRate = 115200;
    huart2.Init.Mode = UART_MODE_TX_RX;
    HAL_UART_Init(&huart2);
    hi2c1.Instance = I2C1;
    hi2c1.Init.ClockSpeed = 400000;
    hi2c1.Init.AddressingMode = I2C_ADDRESSINGMODE_7BIT;
    HAL_I2C_Init(&hi2c1);

    while (HAL_I2C_IsDeviceReady(&hi2c1, MPU_ADDR, 1, 10) != HAL_OK ||
           HAL_I2C_IsDeviceReady(&hi2c1, OLED_ADDR, 1, 10) != HAL_OK) {
        HAL_Delay(100);
    }
    reg_write(0x6B, 0x00); /* MPU6050 : réveil */
    static const uint8_t init[] = {
        0xAE, 0xD5, 0x80, 0xA8, 0x3F, 0xD3, 0x00, 0x40, 0x8D, 0x14, 0x20, 0x00,
        0xA1, 0xC8, 0xDA, 0x12, 0x81, 0xCF, 0xD9, 0xF1, 0xDB, 0x40, 0xA4, 0xA6, 0xAF,
    };
    oled_cmds(init, sizeof init);

    long mg[3] = { 0, 0, 0 };
    for (uint32_t tick = 0;; ++tick) { /* un tour toutes les 50 ms */
        uint8_t d[6];
        if (HAL_I2C_Mem_Read(&hi2c1, MPU_ADDR, 0x3B, I2C_MEMADD_SIZE_8BIT, d, sizeof d, 100) == HAL_OK) {
            for (int i = 0; i < 3; ++i) {
                mg[i] = (long)(int16_t)(((uint16_t)d[2 * i] << 8) | d[2 * i + 1]) * 1000L / 16384L;
            }
        }
        int blink = (HAL_GetTick() / 500u) % 2u == 0u;
        HAL_GPIO_WritePin(GPIOA, GPIO_PIN_5, (blink || pressed) ? GPIO_PIN_SET : GPIO_PIN_RESET);
        if (tick % 2u == 0u) {
            char t[24];
            memset(frame, 0, sizeof frame);
            draw_text(40, 0, "Prebench");
            snprintf(t, sizeof t, "x = %ld mg", mg[0]);
            draw_text(16, 3, t);
            snprintf(t, sizeof t, "y = %ld mg", mg[1]);
            draw_text(16, 5, t);
            snprintf(t, sizeof t, "z = %ld mg", mg[2]);
            draw_text(16, 7, t);
            oled_show();
        }
        if (tick % 4u == 0u) {
            PRINT("ax:%ld,ay:%ld,az:%ld\r\n", mg[0], mg[1], mg[2]);
        }
        HAL_Delay(50);
    }
}
