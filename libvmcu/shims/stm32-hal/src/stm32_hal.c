/*
 * libvmcu/shims/stm32-hal/src/stm32_hal.c — HAL de base, RCC, Cortex/NVIC, CMSIS.
 *
 * SystemClock_Config, HAL_RCC_* et HAL_Init sont acceptés comme fonctions sans
 * effet (section 5.3) ; le temps vient de libvmcu (règle 16.2.3).
 */
#include "stm32_hal_internal.h"

#include <stdatomic.h>

/* Fréquences d'une Nucleo-F401RE configurée par CubeMX (84 MHz). */
#define SIM_SYSCLK_HZ 84000000U

GPIO_TypeDef prebench_gpio_ports[PREBENCH_GPIO_PORT_COUNT];
uint32_t SystemCoreClock = SIM_SYSCLK_HZ;

static uint32_t tick_prio = TICK_INT_PRIORITY;
static HAL_TickFreqTypeDef tick_freq = HAL_TICK_FREQ_DEFAULT;

/* --- HAL de base ----------------------------------------------------------------------- */

HAL_StatusTypeDef HAL_Init(void)
{
    if (vmcu_init(NULL, NULL) != VMCU_OK) {
        return HAL_ERROR;
    }
    HAL_MspInit();
    return HAL_OK;
}

HAL_StatusTypeDef HAL_DeInit(void)
{
    HAL_MspDeInit();
    return HAL_OK;
}

HAL_StatusTypeDef HAL_InitTick(uint32_t TickPriority)
{
    tick_prio = TickPriority;
    return HAL_OK;
}

void HAL_IncTick(void)
{
    /* Le tick vient de vmcu_millis() : rien à incrémenter. */
}

void HAL_Delay(uint32_t Delay)
{
    vmcu_delay_ms(Delay);
}

uint32_t HAL_GetTick(void)
{
    return vmcu_millis();
}

uint32_t HAL_GetTickPrio(void)
{
    return tick_prio;
}

HAL_StatusTypeDef HAL_SetTickFreq(HAL_TickFreqTypeDef Freq)
{
    tick_freq = Freq;
    return HAL_OK;
}

HAL_TickFreqTypeDef HAL_GetTickFreq(void)
{
    return tick_freq;
}

void HAL_SuspendTick(void)
{
    vmcu_warn_unsimulated("HAL_SuspendTick");
}

void HAL_ResumeTick(void)
{
}

uint32_t HAL_GetHalVersion(void)
{
    return 0x01080000U; /* STM32CubeF4 HAL 1.8.0 */
}

uint32_t HAL_GetREVID(void)
{
    return 0x1000U;
}

uint32_t HAL_GetDEVID(void)
{
    return 0x433U; /* STM32F401xD/E */
}

/* --- RCC --------------------------------------------------------------------------------- */

HAL_StatusTypeDef HAL_RCC_OscConfig(RCC_OscInitTypeDef *RCC_OscInitStruct)
{
    UNUSED(RCC_OscInitStruct);
    return HAL_OK;
}

HAL_StatusTypeDef HAL_RCC_ClockConfig(RCC_ClkInitTypeDef *RCC_ClkInitStruct, uint32_t FLatency)
{
    UNUSED(RCC_ClkInitStruct);
    UNUSED(FLatency);
    return HAL_OK;
}

uint32_t HAL_RCC_GetSysClockFreq(void) { return SIM_SYSCLK_HZ; }
uint32_t HAL_RCC_GetHCLKFreq(void)     { return SIM_SYSCLK_HZ; }
uint32_t HAL_RCC_GetPCLK1Freq(void)    { return SIM_SYSCLK_HZ / 2U; }
uint32_t HAL_RCC_GetPCLK2Freq(void)    { return SIM_SYSCLK_HZ; }

void SystemInit(void)
{
}

void SystemCoreClockUpdate(void)
{
}

/* --- NVIC ----------------------------------------------------------------------------------- */

#define NVIC_WORDS 4 /* IRQ 0..127 */
static atomic_uint nvic_enabled[NVIC_WORDS];

bool prebench_nvic_enabled(IRQn_Type irqn)
{
    int n = (int)irqn;
    if (n < 0 || n >= NVIC_WORDS * 32) {
        return false;
    }
    return (atomic_load(&nvic_enabled[n / 32]) & (1U << (n % 32))) != 0U;
}

void HAL_NVIC_SetPriorityGrouping(uint32_t PriorityGroup)
{
    UNUSED(PriorityGroup);
}

void HAL_NVIC_SetPriority(IRQn_Type IRQn, uint32_t PreemptPriority, uint32_t SubPriority)
{
    /* Les priorités ne sont pas simulées : les callbacks sont sérialisés. */
    UNUSED(IRQn);
    UNUSED(PreemptPriority);
    UNUSED(SubPriority);
}

void HAL_NVIC_EnableIRQ(IRQn_Type IRQn)
{
    int n = (int)IRQn;
    if (n < 0 || n >= NVIC_WORDS * 32) {
        return;
    }
    atomic_fetch_or(&nvic_enabled[n / 32], 1U << (n % 32));
    /* Comme sur le vrai NVIC : un front survenu avant l'activation est servi
     * dès l'activation. */
    if (prebench_exti_lines_of(IRQn) != 0U &&
        prebench_exti_get_pending(prebench_exti_lines_of(IRQn)) != 0U) {
        vmcu_irq_lock();
        prebench_exti_call_handler(IRQn);
        vmcu_irq_unlock();
    }
}

void HAL_NVIC_DisableIRQ(IRQn_Type IRQn)
{
    int n = (int)IRQn;
    if (n >= 0 && n < NVIC_WORDS * 32) {
        atomic_fetch_and(&nvic_enabled[n / 32], ~(1U << (n % 32)));
    }
}

void HAL_NVIC_SystemReset(void)
{
    NVIC_SystemReset();
}

uint32_t HAL_SYSTICK_Config(uint32_t TicksNumb)
{
    UNUSED(TicksNumb);
    return 0U;
}

void HAL_SYSTICK_CLKSourceConfig(uint32_t CLKSource)
{
    UNUSED(CLKSource);
}

/* --- Intrinsèques CMSIS ----------------------------------------------------------------------- */

/* PRIMASK par fil : __disable_irq n'est pas imbriqué sur Cortex-M, le verrou
 * libvmcu (récursif) n'est donc pris qu'une fois. */
static _Thread_local bool t_irq_disabled;

void __disable_irq(void)
{
    if (!t_irq_disabled) {
        vmcu_irq_lock();
        t_irq_disabled = true;
    }
}

void __enable_irq(void)
{
    if (t_irq_disabled) {
        t_irq_disabled = false;
        vmcu_irq_unlock();
    }
}

uint32_t __get_PRIMASK(void)
{
    return t_irq_disabled ? 1U : 0U;
}

void __set_PRIMASK(uint32_t primask)
{
    if ((primask & 1U) != 0U) {
        __disable_irq();
    } else {
        __enable_irq();
    }
}

void __WFI(void)
{
    vmcu_delay_ms(1);
}

void __WFE(void)
{
    vmcu_delay_ms(1);
}

void NVIC_SystemReset(void)
{
    vmcu_warn_unsimulated("NVIC_SystemReset");
}
