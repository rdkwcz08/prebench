/*
 * tests/oled-hello/main.c — Écran OLED SSD1306 en I2C (phase 3, étape 3).
 *
 * I2C1 (PB8 SCL, PB9 SDA, AF4), écran en 0x3C, initialisé comme le font les
 * bibliothèques courantes (pompe de charge, A1 + C8, adressage horizontal).
 * Toutes les 100 ms, une image entière (1 024 octets, une seule transaction
 * HAL_I2C_Mem_Write après la fenêtre 0x21 / 0x22) : « Prebench » en haut, le
 * temps écoulé, et une barre qui se déplace. USART2 annonce « oled pret » une
 * fois l'écran initialisé, puis « image:<n> » toutes les 10 images.
 */
#include <stdio.h>
#include <string.h>

#include "stm32f4xx_hal.h"

#define OLED_ADDR (0x3Cu << 1)
#define OLED_CMD  0x00u /* octet de contrôle : suite de commandes */
#define OLED_DATA 0x40u /* octet de contrôle : suite de données */

UART_HandleTypeDef huart2;
I2C_HandleTypeDef hi2c1;

static uint8_t frame[128 * 8]; /* 8 pages de 128 octets verticaux, bit 0 en haut */

/* Police 5 × 7 (colonnes, bit 0 en haut), dessinée pour cet exemple. */
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
};

#define PRINT(...)                                                                  \
    do {                                                                            \
        char line_[48];                                                             \
        int len_ = snprintf(line_, sizeof line_, __VA_ARGS__);                      \
        HAL_UART_Transmit(&huart2, (const uint8_t *)line_, (uint16_t)len_, 100);    \
    } while (0)

static void oled_cmds(const uint8_t *cmds, uint16_t n)
{
    HAL_I2C_Mem_Write(&hi2c1, OLED_ADDR, OLED_CMD, I2C_MEMADD_SIZE_8BIT, (uint8_t *)cmds, n, 100);
}

static void oled_init(void)
{
    static const uint8_t init[] = {
        0xAE,             /* écran éteint */
        0xD5, 0x80,       /* horloge */
        0xA8, 0x3F,       /* multiplexage 64 lignes */
        0xD3, 0x00,       /* décalage 0 */
        0x40,             /* ligne de départ 0 */
        0x8D, 0x14,       /* pompe de charge */
        0x20, 0x00,       /* adressage horizontal */
        0xA1, 0xC8,       /* segments et lignes inversés : image à l'endroit */
        0xDA, 0x12,       /* broches COM */
        0x81, 0xCF,       /* contraste */
        0xD9, 0xF1,       /* précharge */
        0xDB, 0x40,       /* VCOMH */
        0xA4, 0xA6,       /* image de la mémoire, non inversée */
        0xAF,             /* écran allumé */
    };
    oled_cmds(init, sizeof init);
}

static void oled_show(void)
{
    static const uint8_t window[] = { 0x21, 0, 127, 0x22, 0, 7 };
    oled_cmds(window, sizeof window);
    HAL_I2C_Mem_Write(&hi2c1, OLED_ADDR, OLED_DATA, I2C_MEMADD_SIZE_8BIT, frame, sizeof frame, 100);
}

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

int main(void)
{
    HAL_Init();
    __HAL_RCC_USART2_CLK_ENABLE();
    huart2.Instance = USART2;
    huart2.Init.BaudRate = 115200;
    huart2.Init.Mode = UART_MODE_TX_RX;
    HAL_UART_Init(&huart2);
    hi2c1.Instance = I2C1;
    hi2c1.Init.ClockSpeed = 400000;
    hi2c1.Init.AddressingMode = I2C_ADDRESSINGMODE_7BIT;
    HAL_I2C_Init(&hi2c1);

    /* Attend que l'écran réponde (module alimenté et câblé). */
    while (HAL_I2C_IsDeviceReady(&hi2c1, OLED_ADDR, 1, 10) != HAL_OK) {
        HAL_Delay(100);
    }
    oled_init();
    PRINT("oled pret\r\n");

    unsigned n = 0;
    while (1) {
        memset(frame, 0, sizeof frame);
        draw_text(40, 0, "Prebench");
        char t[24];
        uint32_t ms = HAL_GetTick();
        snprintf(t, sizeof t, "t = %lu.%lu s", (unsigned long)(ms / 1000u), (unsigned long)(ms / 100u % 10u));
        draw_text(28, 3, t);
        unsigned bar = (n * 4u) % 104u; /* barre de 24 pixels qui avance de 4 par image */
        for (unsigned x = bar; x < bar + 24u; ++x) {
            frame[6 * 128 + x] = 0xFF;
        }
        oled_show();
        if (++n % 10u == 0u) {
            PRINT("image:%u\r\n", n);
        }
        HAL_Delay(100);
    }
}
