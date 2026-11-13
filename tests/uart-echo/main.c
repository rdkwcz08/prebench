/*
 * tests/uart-echo/main.c — Moniteur série et traceur (UART du HAL STM32).
 *
 * USART2 à 115 200 bauds, comme le port série du ST-LINK de la Nucleo :
 * - chaque octet reçu est renvoyé (réception _IT d'un octet, relancée dans
 *   HAL_UART_RxCpltCallback, rangée dans un tampon circulaire que la boucle
 *   principale vide : un octet unique serait écrasé par le suivant) ;
 * - toutes les 500 ms, une mesure au format du traceur : « temp:21.4,hum:55 ».
 */
#include <stdio.h>
#include <string.h>

#include "stm32f4xx_hal.h"

UART_HandleTypeDef huart2;
static uint8_t rx_byte;
static volatile uint8_t ring[64];
static volatile uint8_t head, tail; /* tampon circulaire : écrit par le callback, lu par main */

void HAL_UART_RxCpltCallback(UART_HandleTypeDef *huart)
{
    if (huart == &huart2) {
        uint8_t next = (uint8_t)((head + 1u) % sizeof ring);
        if (next != tail) {
            ring[head] = rx_byte;
            head = next;
        }
        (void)HAL_UART_Receive_IT(&huart2, &rx_byte, 1);
    }
}

int main(void)
{
    HAL_Init();
    __HAL_RCC_USART2_CLK_ENABLE();
    huart2.Instance = USART2;
    huart2.Init.BaudRate = 115200;
    huart2.Init.WordLength = UART_WORDLENGTH_8B;
    huart2.Init.StopBits = UART_STOPBITS_1;
    huart2.Init.Parity = UART_PARITY_NONE;
    huart2.Init.Mode = UART_MODE_TX_RX;
    HAL_UART_Init(&huart2);
    (void)HAL_UART_Receive_IT(&huart2, &rx_byte, 1);

    const char hello[] = "Prebench : écho USART2 prêt\r\n";
    HAL_UART_Transmit(&huart2, (const uint8_t *)hello, (uint16_t)strlen(hello), 100);

    uint32_t next = HAL_GetTick();
    unsigned n = 0;
    while (1) {
        while (tail != head) {
            uint8_t b = ring[tail];
            tail = (uint8_t)((tail + 1u) % sizeof ring);
            HAL_UART_Transmit(&huart2, &b, 1, 10);
        }
        if ((int32_t)(HAL_GetTick() - next) >= 0) {
            next += 500;
            char line[48];
            int len = snprintf(line, sizeof line, "temp:%u.%u,hum:%u\r\n", 21u + n % 3u, n % 10u, 50u + n % 7u);
            HAL_UART_Transmit(&huart2, (const uint8_t *)line, (uint16_t)len, 10);
            n++;
        }
        HAL_Delay(1);
    }
}
