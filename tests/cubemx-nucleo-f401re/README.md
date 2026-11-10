# Projet de test STM32CubeMX — Nucleo-F401RE, configuration par défaut

Ce dossier sert au critère de passage de la phase 1 (cahier des charges, section 14). Il contient un projet STM32CubeMX **non modifié**, que la cible « sim » doit compiler tel quel.

## État actuel

Les fichiers de `Core/` sont pour l'instant un **équivalent écrit à la main** du code que CubeMX génère pour une Nucleo-F401RE en configuration par défaut :

- GPIO : LD2 sur PA5 en sortie, B1 sur PC13 en interruption sur front descendant ;
- USART2 à 115 200 bauds sur PA2 / PA3 ;
- horloge à 84 MHz, à partir du HSI et de la PLL.

Ils seront remplacés par le projet réellement généré.

## Remplacer par le vrai projet

1. Dans STM32CubeMX : **New Project → Board Selector → NUCLEO-F401RE**, puis répondre **Yes** à « Initialize all peripherals with their default Mode ».
2. Dans **Project Manager** :
   - Project Name : `cubemx-nucleo-f401re` ;
   - Project Location : le dossier `tests/` du dépôt ;
   - Toolchain / IDE : **CMake**.
3. Cliquer **Generate Code**, sans rien modifier ensuite.
4. Depuis la racine du dépôt, vérifier :

   ```sh
   cmake -S libvmcu -B build-libvmcu -DVMCU_WERROR=ON
   cmake --build build-libvmcu --parallel
   ctest --test-dir build-libvmcu --output-on-failure -R test_cubemx
   ```

## Ce que compile la cible « sim »

`libvmcu/cmake/PrebenchFirmware.cmake` :

| Élément | Traitement |
| --- | --- |
| `Core/Src/*.c` | Compilés tels quels |
| `system_stm32f4xx.c`, `syscalls.c`, `sysmem.c` | Exclus : registres CMSIS, newlib et symboles de l'éditeur de liens propres à la vraie puce |
| `startup_stm32f401xe.s`, `*.ld`, `Drivers/` | Ignorés : le shim STM32 HAL remplace le HAL et CMSIS |
| `Core/Inc` | Ajouté aux en-têtes, avant ceux du shim |

Build « sim » autonome du projet, depuis la racine du dépôt :

```sh
cmake -S libvmcu/sim -B build-sim -DPREBENCH_PROJECT=tests/cubemx-nucleo-f401re
cmake --build build-sim
```

Sous Windows, ajouter `-G "MinGW Makefiles"` à la première commande.

## Comportement attendu en phase 1

- **Simulés** : LD2, B1, l'interruption EXTI13 et le temps.
- **USART2 : non simulé.** `HAL_UART_Init` appelle `HAL_UART_MspInit` du projet, ce qui configure PA2 et PA3 en AF7, puis avertit « HAL_UART_Init non simulé ». Le moniteur série arrive en phase 2.
- **Boucle principale.** La boucle `while (1) {}` de CubeMX n'appelle jamais libvmcu : elle occupe un cœur à 100 %. Les micro-pauses de libvmcu ne peuvent pas agir sans appel à libvmcu. Le plafond CPU par processus (section 11.2) arrive en phase 2.
