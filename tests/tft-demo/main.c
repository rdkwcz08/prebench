/*
 * tests/tft-demo/main.c — Écran TFT ILI9341 sur SPI1 (phase 3, étape 7).
 *
 * Écrit comme un projet STM32CubeMX avec un pilote ILI9341 minimal (celui des
 * bibliothèques courantes) : CS sur PB6 (D10), DC sur PC7 (D9), RESET sur PA9
 * (D8), SPI1 maître en mode 0. Pixels RGB565, octet de poids fort d'abord.
 *
 * Pour chacune des quatre orientations (MADCTL 0x48, 0x28, 0x88, 0xE8, celles
 * des bibliothèques Adafruit et STM32), la mire :
 *   - quatre barres en haut : rouge 0xF800, vert 0x07E0, bleu 0x001F, blanc ;
 *   - trois dégradés de gauche à droite : rouge (0 à 31), vert (0 à 63), bleu ;
 *   - repères 12 × 12 : magenta en haut à gauche, cyan en haut à droite ;
 * puis un carré blanc de 12 × 12 rebondit dans le bas de l'image pendant 2 s :
 * à chaque pas, seules l'ancienne et la nouvelle position sont réécrites
 * (fenêtres CASET × PASET), jamais l'écran entier.
 * Les écritures de plus de 4096 octets (bandes de 20 lignes) passent par la
 * mémoire partagée.
 */
#include <stdio.h>
#include <string.h>

#include "stm32f4xx_hal.h"

#define CS_PORT  GPIOB
#define CS_PIN   GPIO_PIN_6
#define DC_PORT  GPIOC
#define DC_PIN   GPIO_PIN_7
#define RST_PORT GPIOA
#define RST_PIN  GPIO_PIN_9

#define RED     0xF800u
#define GREEN   0x07E0u
#define BLUE    0x001Fu
#define WHITE   0xFFFFu
#define BLACK   0x0000u
#define MAGENTA 0xF81Fu
#define CYAN    0x07FFu

#define BAND_LINES 20u

UART_HandleTypeDef huart2;
SPI_HandleTypeDef hspi1;

static uint16_t width = 240;
static uint16_t height = 320;
/* Bande de 20 lignes de 320 pixels au plus, octets dans l'ordre du fil. */
static uint8_t band[320u * BAND_LINES * 2u];

#define PRINT(...)                                                                  \
    do {                                                                            \
        char line_[80];                                                             \
        int len_ = snprintf(line_, sizeof line_, __VA_ARGS__);                      \
        HAL_UART_Transmit(&huart2, (const uint8_t *)line_, (uint16_t)len_, 100);    \
    } while (0)

/* --- Pilote ILI9341 minimal -------------------------------------------------------------- */

static void tft_command(uint8_t c)
{
    HAL_GPIO_WritePin(DC_PORT, DC_PIN, GPIO_PIN_RESET);
    HAL_GPIO_WritePin(CS_PORT, CS_PIN, GPIO_PIN_RESET);
    HAL_SPI_Transmit(&hspi1, &c, 1, 10);
    HAL_GPIO_WritePin(CS_PORT, CS_PIN, GPIO_PIN_SET);
}

static void tft_data(const uint8_t *data, uint32_t len)
{
    HAL_GPIO_WritePin(DC_PORT, DC_PIN, GPIO_PIN_SET);
    HAL_GPIO_WritePin(CS_PORT, CS_PIN, GPIO_PIN_RESET);
    while (len > 0) {
        uint16_t n = len > 0xFFFFu ? 0xFFFFu : (uint16_t)len;
        HAL_SPI_Transmit(&hspi1, data, n, 100);
        data += n;
        len -= n;
    }
    HAL_GPIO_WritePin(CS_PORT, CS_PIN, GPIO_PIN_SET);
}

static void tft_command_args(uint8_t c, const uint8_t *args, uint32_t n)
{
    tft_command(c);
    if (n > 0) {
        tft_data(args, n);
    }
}

static void tft_window(uint16_t x, uint16_t y, uint16_t w, uint16_t h)
{
    uint16_t x1 = (uint16_t)(x + w - 1), y1 = (uint16_t)(y + h - 1);
    uint8_t col[4] = { (uint8_t)(x >> 8), (uint8_t)x, (uint8_t)(x1 >> 8), (uint8_t)x1 };
    uint8_t row[4] = { (uint8_t)(y >> 8), (uint8_t)y, (uint8_t)(y1 >> 8), (uint8_t)y1 };
    tft_command_args(0x2A, col, 4); /* CASET */
    tft_command_args(0x2B, row, 4); /* PASET */
    tft_command(0x2C);              /* RAMWR */
}

static void put_pixel(uint8_t *at, uint16_t color)
{
    at[0] = (uint8_t)(color >> 8);
    at[1] = (uint8_t)color;
}

/* Rectangle uni, par bandes de 20 lignes. */
static void tft_fill(uint16_t x, uint16_t y, uint16_t w, uint16_t h, uint16_t color)
{
    tft_window(x, y, w, h);
    uint32_t lines = h < BAND_LINES ? h : BAND_LINES;
    for (uint32_t i = 0; i < (uint32_t)w * lines; ++i) {
        put_pixel(band + 2 * i, color);
    }
    for (uint32_t done = 0; done < h; done += lines) {
        uint32_t n = h - done < lines ? h - done : lines;
        tft_data(band, (uint32_t)w * n * 2u);
    }
}

/* Dégradé horizontal d'une composante (0 = rouge, 1 = vert, 2 = bleu). */
static void tft_gradient(uint16_t y, uint16_t h, int channel)
{
    uint32_t max = channel == 1 ? 63u : 31u;
    for (uint32_t x = 0; x < width; ++x) {
        uint16_t v = (uint16_t)(x * max / (width - 1u));
        uint16_t color = channel == 0 ? (uint16_t)(v << 11) : channel == 1 ? (uint16_t)(v << 5) : v;
        put_pixel(band + 2 * x, color);
    }
    tft_window(0, y, width, h);
    for (uint16_t i = 0; i < h; ++i) {
        tft_data(band, (uint32_t)width * 2u);
    }
}

static void tft_init(void)
{
    HAL_GPIO_WritePin(RST_PORT, RST_PIN, GPIO_PIN_RESET);
    HAL_Delay(5);
    HAL_GPIO_WritePin(RST_PORT, RST_PIN, GPIO_PIN_SET);
    HAL_Delay(120);
    tft_command(0x01); /* SWRESET */
    HAL_Delay(150);
    /* Réglages d'alimentation et de gamma des pilotes courants (sans effet simulé). */
    tft_command_args(0xCF, (const uint8_t[]){ 0x00, 0xC1, 0x30 }, 3);
    tft_command_args(0xC0, (const uint8_t[]){ 0x23 }, 1);
    tft_command_args(0xC1, (const uint8_t[]){ 0x10 }, 1);
    tft_command_args(0xC5, (const uint8_t[]){ 0x3E, 0x28 }, 2);
    tft_command_args(0xB1, (const uint8_t[]){ 0x00, 0x18 }, 2);
    tft_command_args(0x3A, (const uint8_t[]){ 0x55 }, 1); /* COLMOD : 16 bits */
    tft_command_args(0x36, (const uint8_t[]){ 0x48 }, 1); /* MADCTL : MX, BGR */
    tft_command(0x11); /* SLPOUT */
    HAL_Delay(120);
    tft_command(0x29); /* DISPON */
}

static void tft_rotation(uint8_t madctl)
{
    tft_command_args(0x36, &madctl, 1);
    int swap = (madctl & 0x20u) != 0;
    width = swap ? 320 : 240;
    height = swap ? 240 : 320;
}

/* Identifiant lu par 0xD3 (RDID4) : 0x9341 attendu. */
static uint16_t tft_read_id(void)
{
    uint8_t c = 0xD3, rx[4] = { 0 }, tx[4] = { 0 };
    HAL_GPIO_WritePin(CS_PORT, CS_PIN, GPIO_PIN_RESET);
    HAL_GPIO_WritePin(DC_PORT, DC_PIN, GPIO_PIN_RESET);
    HAL_SPI_Transmit(&hspi1, &c, 1, 10);
    HAL_GPIO_WritePin(DC_PORT, DC_PIN, GPIO_PIN_SET);
    HAL_SPI_TransmitReceive(&hspi1, tx, rx, 4, 10);
    HAL_GPIO_WritePin(CS_PORT, CS_PIN, GPIO_PIN_SET);
    return (uint16_t)((rx[2] << 8) | rx[3]);
}

static void mire(void)
{
    tft_fill(0, 0, width, height, BLACK);
    uint16_t bar = (uint16_t)(width / 4u), bar_h = (uint16_t)(height / 4u);
    const uint16_t colors[4] = { RED, GREEN, BLUE, WHITE };
    for (int i = 0; i < 4; ++i) {
        tft_fill((uint16_t)(i * bar), 0, bar, bar_h, colors[i]);
    }
    uint16_t g = (uint16_t)(height / 8u);
    for (int ch = 0; ch < 3; ++ch) {
        tft_gradient((uint16_t)(bar_h + ch * g), g, ch);
    }
    tft_fill(0, 0, 12, 12, MAGENTA);
    tft_fill((uint16_t)(width - 12u), 0, 12, 12, CYAN);
}

/* --- CubeMX ------------------------------------------------------------------------------ */

void HAL_SPI_MspInit(SPI_HandleTypeDef *hspi)
{
    if (hspi->Instance == SPI1) {
        GPIO_InitTypeDef gpio = { 0 };
        __HAL_RCC_SPI1_CLK_ENABLE();
        __HAL_RCC_GPIOA_CLK_ENABLE();
        gpio.Pin = GPIO_PIN_5 | GPIO_PIN_6 | GPIO_PIN_7;
        gpio.Mode = GPIO_MODE_AF_PP;
        gpio.Pull = GPIO_NOPULL;
        gpio.Speed = GPIO_SPEED_FREQ_VERY_HIGH;
        gpio.Alternate = GPIO_AF5_SPI1;
        HAL_GPIO_Init(GPIOA, &gpio);
    }
}

static void MX_GPIO_Init(void)
{
    GPIO_InitTypeDef gpio = { 0 };
    __HAL_RCC_GPIOA_CLK_ENABLE();
    __HAL_RCC_GPIOB_CLK_ENABLE();
    __HAL_RCC_GPIOC_CLK_ENABLE();
    HAL_GPIO_WritePin(CS_PORT, CS_PIN, GPIO_PIN_SET);
    HAL_GPIO_WritePin(DC_PORT, DC_PIN, GPIO_PIN_SET);
    HAL_GPIO_WritePin(RST_PORT, RST_PIN, GPIO_PIN_SET);
    gpio.Mode = GPIO_MODE_OUTPUT_PP;
    gpio.Pull = GPIO_NOPULL;
    gpio.Speed = GPIO_SPEED_FREQ_HIGH;
    gpio.Pin = CS_PIN;
    HAL_GPIO_Init(CS_PORT, &gpio);
    gpio.Pin = DC_PIN;
    HAL_GPIO_Init(DC_PORT, &gpio);
    gpio.Pin = RST_PIN;
    HAL_GPIO_Init(RST_PORT, &gpio);
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

static void MX_SPI1_Init(void)
{
    hspi1.Instance = SPI1;
    hspi1.Init.Mode = SPI_MODE_MASTER;
    hspi1.Init.Direction = SPI_DIRECTION_2LINES;
    hspi1.Init.DataSize = SPI_DATASIZE_8BIT;
    hspi1.Init.CLKPolarity = SPI_POLARITY_LOW;
    hspi1.Init.CLKPhase = SPI_PHASE_1EDGE;
    hspi1.Init.NSS = SPI_NSS_SOFT;
    hspi1.Init.BaudRatePrescaler = SPI_BAUDRATEPRESCALER_2;
    hspi1.Init.FirstBit = SPI_FIRSTBIT_MSB;
    hspi1.Init.TIMode = SPI_TIMODE_DISABLE;
    hspi1.Init.CRCCalculation = SPI_CRCCALCULATION_DISABLE;
    hspi1.Init.CRCPolynomial = 10;
    HAL_SPI_Init(&hspi1);
}

int main(void)
{
    HAL_Init();
    MX_GPIO_Init();
    MX_USART2_UART_Init();
    MX_SPI1_Init();
    tft_init();
    PRINT("ILI9341 : identifiant 0x%04X\r\n", (unsigned)tft_read_id());

    static const uint8_t rotations[4] = { 0x48, 0x28, 0x88, 0xE8 };
    for (unsigned r = 0;; r = (r + 1u) % 4u) {
        tft_rotation(rotations[r]);
        PRINT("orientation %u (MADCTL 0x%02X) : %u x %u\r\n", r, rotations[r], width, height);
        mire();
        /* Carré rebondissant dans le bas de l'image, 30 pas par seconde. */
        int x = 20, y = (int)(height * 3u / 4u), dx = 4, dy = 3;
        int top = (int)(height / 4u + 3u * (height / 8u)) + 4, bottom = (int)height - 12;
        for (int step = 0; step < 60; ++step) {
            tft_fill((uint16_t)x, (uint16_t)y, 12, 12, BLACK);
            x += dx;
            y += dy;
            if (x < 0 || x > (int)width - 12) {
                dx = -dx;
                x += 2 * dx;
            }
            if (y < top || y > bottom) {
                dy = -dy;
                y += 2 * dy;
            }
            tft_fill((uint16_t)x, (uint16_t)y, 12, 12, WHITE);
            HAL_Delay(33);
        }
    }
}
