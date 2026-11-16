/*
 * tests/delay-accuracy/main.c — Exactitude des délais (cahier des charges, section 5.5).
 *
 * Le firmware tourne en priorité basse, comme dans l'app (QoS « utility » sur
 * macOS, EcoQoS et BELOW_NORMAL sous Windows, nice 10 sous Linux). Trois
 * séries de 100 HAL_Delay(10) : chacune doit durer environ 1 s, mesurée par
 * HAL_GetTick et envoyée sur USART2 (« delai:<ms> »).
 */
#include <stdio.h>

#include "stm32f4xx_hal.h"

UART_HandleTypeDef huart2;

int main(void)
{
    HAL_Init();
    huart2.Instance = USART2;
    huart2.Init.BaudRate = 115200;
    huart2.Init.Mode = UART_MODE_TX_RX;
    HAL_UART_Init(&huart2);
    HAL_Delay(200); /* mise en route */
    for (int serie = 0; serie < 3; ++serie) {
        uint32_t t0 = HAL_GetTick();
        for (int i = 0; i < 100; ++i) {
            HAL_Delay(10);
        }
        char line[32];
        int len = snprintf(line, sizeof line, "delai:%lu\r\n", (unsigned long)(HAL_GetTick() - t0));
        HAL_UART_Transmit(&huart2, (uint8_t *)line, (uint16_t)len, 100);
    }
    while (1) {
        HAL_Delay(1000);
    }
}
