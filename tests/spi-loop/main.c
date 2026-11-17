/*
 * tests/spi-loop/main.c — Bus SPI du HAL STM32 (phase 3, étape 6).
 *
 * Écrit comme un projet STM32CubeMX : MX_SPI1_Init (maître, mode 0, NSS
 * logiciel), HAL_SPI_MspInit (PA5 SCK, PA6 MISO, PA7 MOSI en AF5), CS sur PB6
 * (D10) en sortie GPIO, au repos à l'état haut. Face au composant d'écho, qui
 * renvoie chaque octet reçu :
 * 1. Boucle : CS bas, HAL_SPI_TransmitReceive, CS haut ; octets relus identiques.
 * 2. Sans CS : aucun esclave sélectionné, MISO lu à 0xFF.
 * 3. Trame de 60 000 octets par HAL_SPI_Transmit_DMA (au-delà de 4096 octets,
 *    les données passent par la mémoire partagée) ; CS relâché dans
 *    HAL_SPI_TxCpltCallback, après le demi-transfert.
 * 4. Toutes les 250 ms : HAL_SPI_TransmitReceive_IT d'un compteur ;
 *    HAL_SPI_TxRxCpltCallback relâche CS et lève un drapeau, la boucle
 *    principale envoie « boucle:<valeur> » au traceur.
 */
#include <stdio.h>
#include <string.h>

#include "stm32f4xx_hal.h"

#define CS_PORT GPIOB
#define CS_PIN  GPIO_PIN_6

#define FRAME_SIZE 60000u

UART_HandleTypeDef huart2;
SPI_HandleTypeDef hspi1;

static uint8_t frame[FRAME_SIZE];
static uint8_t counter_tx;
static uint8_t counter_rx;
static volatile int half_done;
static volatile int frame_done;
static volatile int counter_ready;
static volatile int spi_error;

/* Ligne de texte sur USART2 (moniteur série et traceur). */
#define PRINT(...)                                                                  \
    do {                                                                            \
        char line_[64];                                                             \
        int len_ = snprintf(line_, sizeof line_, __VA_ARGS__);                      \
        HAL_UART_Transmit(&huart2, (const uint8_t *)line_, (uint16_t)len_, 100);    \
    } while (0)

static void cs(GPIO_PinState level)
{
    HAL_GPIO_WritePin(CS_PORT, CS_PIN, level);
}

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
        HAL_NVIC_SetPriority(SPI1_IRQn, 0, 0);
        HAL_NVIC_EnableIRQ(SPI1_IRQn);
    }
}

void HAL_SPI_TxHalfCpltCallback(SPI_HandleTypeDef *hspi)
{
    if (hspi == &hspi1) {
        half_done = 1;
    }
}

void HAL_SPI_TxCpltCallback(SPI_HandleTypeDef *hspi)
{
    if (hspi == &hspi1) {
        cs(GPIO_PIN_SET);
        frame_done = 1;
    }
}

void HAL_SPI_TxRxCpltCallback(SPI_HandleTypeDef *hspi)
{
    if (hspi == &hspi1) {
        cs(GPIO_PIN_SET);
        counter_ready = 1;
    }
}

void HAL_SPI_ErrorCallback(SPI_HandleTypeDef *hspi)
{
    if (hspi == &hspi1) {
        cs(GPIO_PIN_SET);
        spi_error = 1;
    }
}

static void MX_GPIO_Init(void)
{
    GPIO_InitTypeDef gpio = { 0 };
    __HAL_RCC_GPIOB_CLK_ENABLE();
    HAL_GPIO_WritePin(CS_PORT, CS_PIN, GPIO_PIN_SET);
    gpio.Pin = CS_PIN;
    gpio.Mode = GPIO_MODE_OUTPUT_PP;
    gpio.Pull = GPIO_NOPULL;
    gpio.Speed = GPIO_SPEED_FREQ_HIGH;
    HAL_GPIO_Init(CS_PORT, &gpio);
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
    hspi1.Init.BaudRatePrescaler = SPI_BAUDRATEPRESCALER_16;
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

    /* 1. Boucle */
    const uint8_t msg[] = "Prebench SPI";
    uint8_t back[sizeof msg] = { 0 };
    cs(GPIO_PIN_RESET);
    HAL_StatusTypeDef st = HAL_SPI_TransmitReceive(&hspi1, msg, back, sizeof msg, 100);
    cs(GPIO_PIN_SET);
    PRINT("boucle : %s\r\n", (st == HAL_OK && memcmp(msg, back, sizeof msg) == 0) ? "OK" : "erreur");

    /* 2. Sans CS : personne ne répond */
    memset(back, 0, sizeof back);
    HAL_SPI_TransmitReceive(&hspi1, msg, back, sizeof msg, 100);
    PRINT("sans CS : MISO 0x%02X\r\n", back[0]);

    /* 3. Trame de 60 000 octets par DMA */
    for (uint32_t i = 0; i < FRAME_SIZE; ++i) {
        frame[i] = (uint8_t)i;
    }
    cs(GPIO_PIN_RESET);
    if (HAL_SPI_Transmit_DMA(&hspi1, frame, (uint16_t)FRAME_SIZE) != HAL_OK) {
        cs(GPIO_PIN_SET);
        PRINT("trame : refusée\r\n");
    }
    while (!frame_done && !spi_error) {
        HAL_Delay(1);
    }
    PRINT("trame : %u octets%s\r\n", (unsigned)FRAME_SIZE, half_done ? ", demi-transfert vu" : "");

    /* 4. Relecture d'un compteur par interruption, 4 fois par seconde */
    uint32_t next = HAL_GetTick();
    while (1) {
        if ((int32_t)(HAL_GetTick() - next) >= 0 && HAL_SPI_GetState(&hspi1) == HAL_SPI_STATE_READY) {
            next += 250;
            counter_tx++;
            cs(GPIO_PIN_RESET);
            (void)HAL_SPI_TransmitReceive_IT(&hspi1, &counter_tx, &counter_rx, 1);
        }
        if (counter_ready) {
            counter_ready = 0;
            PRINT("boucle:%u\r\n", counter_rx);
        }
        if (spi_error) {
            spi_error = 0;
            PRINT("erreur SPI 0x%02X\r\n", (unsigned)HAL_SPI_GetError(&hspi1));
        }
        HAL_Delay(1);
    }
}
