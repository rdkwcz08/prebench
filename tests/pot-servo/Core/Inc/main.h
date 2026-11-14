/* USER CODE BEGIN Header */
/**
  ******************************************************************************
  * @file           : main.h
  * @brief          : Header for main.c file.
  ******************************************************************************
  * Exemple Prebench pot-servo, structure STM32CubeMX (Nucleo-F401RE).
  ******************************************************************************
  */
/* USER CODE END Header */

#ifndef __MAIN_H
#define __MAIN_H

#ifdef __cplusplus
extern "C" {
#endif

#include "stm32f4xx_hal.h"

void HAL_TIM_MspPostInit(TIM_HandleTypeDef *htim);
void Error_Handler(void);

/* Broches (noms de STM32CubeMX) */
#define POT_Pin GPIO_PIN_0
#define POT_GPIO_Port GPIOA
#define SERVO_Pin GPIO_PIN_4
#define SERVO_GPIO_Port GPIOB
#define USART_TX_Pin GPIO_PIN_2
#define USART_RX_Pin GPIO_PIN_3
#define USART_GPIO_Port GPIOA

#ifdef __cplusplus
}
#endif

#endif /* __MAIN_H */
