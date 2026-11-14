/*
 * libvmcu/shims/stm32-hal/src/stm32_hal.c — HAL de base, RCC, Cortex/NVIC, CMSIS.
 *
 * SystemClock_Config, HAL_RCC_* et HAL_Init sont acceptés sans effet sur le
 * temps (section 5.3) : le temps vient de libvmcu (règle 16.2.3). L'horloge
 * n'est pas simulée (règle 16.2.1), mais ses fréquences sont déduites des
 * paramètres passés à HAL_RCC_OscConfig (PLL) et HAL_RCC_ClockConfig (source,
 * diviseurs AHB et APB), comme le calculerait la puce : elles servent à la
 * fréquence des PWM (horloge des timers) et à HAL_RCC_Get*Freq.
 */
#include "stm32_hal_internal.h"

#include <stdatomic.h>

/* Oscillateurs de la Nucleo-F401RE : HSI interne 16 MHz ; HSE 8 MHz fourni
 * par le ST-LINK (MCO), si le projet ne définit pas HSE_VALUE. */
#define SIM_HSI_HZ 16000000U
#ifndef HSE_VALUE
#define HSE_VALUE 8000000U
#endif

GPIO_TypeDef prebench_gpio_ports[PREBENCH_GPIO_PORT_COUNT];
/* Au démarrage, comme sur la puce : HSI à 16 MHz, sans diviseur. */
uint32_t SystemCoreClock = SIM_HSI_HZ;

static struct {
    uint64_t pll_hz;     /* sortie P de la PLL, 0 si arrêtée ou inconnue */
    uint32_t source;     /* RCC_SYSCLKSOURCE_* */
    uint32_t ahb_div;    /* 1..512 */
    uint32_t apb1_div;   /* 1..16 */
    uint32_t apb2_div;
} clocks = { 0, RCC_SYSCLKSOURCE_HSI, 1, 1, 1 };

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

/* Diviseur AHB (RCC_SYSCLK_DIVx) : 0 -> 1, 0x80 -> 2, 0x90 -> 4 … 0xF0 -> 512 (64 saute 32). */
static uint32_t ahb_divider(uint32_t code)
{
    static const uint32_t div[8] = { 2U, 4U, 8U, 16U, 64U, 128U, 256U, 512U };
    return (code & 0x80U) != 0U ? div[(code >> 4) & 0x7U] : 1U;
}

/* Diviseur APB (RCC_HCLK_DIVx) : 0 -> 1, 0x1000 -> 2, 0x1400 -> 4, 0x1800 -> 8, 0x1C00 -> 16. */
static uint32_t apb_divider(uint32_t code)
{
    return (code & 0x1000U) != 0U ? 2U << ((code >> 10) & 0x3U) : 1U;
}

HAL_StatusTypeDef HAL_RCC_OscConfig(RCC_OscInitTypeDef *RCC_OscInitStruct)
{
    if (RCC_OscInitStruct == NULL) {
        return HAL_ERROR;
    }
    const RCC_PLLInitTypeDef *pll = &RCC_OscInitStruct->PLL;
    if (pll->PLLState == RCC_PLL_ON) {
        uint64_t in = (pll->PLLSource == RCC_PLLSOURCE_HSE) ? HSE_VALUE : SIM_HSI_HZ;
        if (pll->PLLM == 0U || pll->PLLP == 0U) {
            return HAL_ERROR; /* la vraie puce refuse aussi ces valeurs */
        }
        clocks.pll_hz = in / pll->PLLM * pll->PLLN / pll->PLLP;
    } else if (pll->PLLState == RCC_PLL_OFF) {
        clocks.pll_hz = 0;
    }
    return HAL_OK;
}

uint32_t HAL_RCC_GetSysClockFreq(void)
{
    switch (clocks.source) {
    case RCC_SYSCLKSOURCE_HSE:    return HSE_VALUE;
    case RCC_SYSCLKSOURCE_PLLCLK: return (uint32_t)clocks.pll_hz;
    default:                      return SIM_HSI_HZ;
    }
}

uint32_t HAL_RCC_GetHCLKFreq(void)  { return HAL_RCC_GetSysClockFreq() / clocks.ahb_div; }
uint32_t HAL_RCC_GetPCLK1Freq(void) { return HAL_RCC_GetHCLKFreq() / clocks.apb1_div; }
uint32_t HAL_RCC_GetPCLK2Freq(void) { return HAL_RCC_GetHCLKFreq() / clocks.apb2_div; }

HAL_StatusTypeDef HAL_RCC_ClockConfig(RCC_ClkInitTypeDef *RCC_ClkInitStruct, uint32_t FLatency)
{
    UNUSED(FLatency);
    if (RCC_ClkInitStruct == NULL) {
        return HAL_ERROR;
    }
    const RCC_ClkInitTypeDef *c = RCC_ClkInitStruct;
    if ((c->ClockType & RCC_CLOCKTYPE_SYSCLK) != 0U) {
        if (c->SYSCLKSource == RCC_SYSCLKSOURCE_PLLCLK && clocks.pll_hz == 0U) {
            return HAL_ERROR; /* PLL non démarrée : la puce refuserait de basculer */
        }
        clocks.source = c->SYSCLKSource;
    }
    if ((c->ClockType & RCC_CLOCKTYPE_HCLK) != 0U) {
        clocks.ahb_div = ahb_divider(c->AHBCLKDivider);
    }
    if ((c->ClockType & RCC_CLOCKTYPE_PCLK1) != 0U) {
        clocks.apb1_div = apb_divider(c->APB1CLKDivider);
    }
    if ((c->ClockType & RCC_CLOCKTYPE_PCLK2) != 0U) {
        clocks.apb2_div = apb_divider(c->APB2CLKDivider);
    }
    SystemCoreClock = HAL_RCC_GetHCLKFreq();
    return HAL_OK;
}

uint32_t prebench_tim_clock_hz(bool apb2)
{
    /* Un bus APB divisé fournit aux timers le double de sa fréquence. */
    uint32_t div = apb2 ? clocks.apb2_div : clocks.apb1_div;
    uint32_t pclk = apb2 ? HAL_RCC_GetPCLK2Freq() : HAL_RCC_GetPCLK1Freq();
    return (div == 1U) ? pclk : 2U * pclk;
}

void SystemInit(void)
{
}

void SystemCoreClockUpdate(void)
{
    SystemCoreClock = HAL_RCC_GetHCLKFreq();
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
