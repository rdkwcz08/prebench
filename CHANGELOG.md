# Journal des modifications

Les évolutions de Prebench qui comptent pour les utilisateurs sont consignées ici.

Format inspiré de [Keep a Changelog](https://keepachangelog.com/fr/1.1.0/). Versions [SemVer](https://semver.org/lang/fr/) en `0.x` jusqu'à la stabilité. `release.sh` exige une section `## [X.Y.Z]` avant de publier une version.

## [Non publié]

## [0.1.0] - 2026-11-10

**Aperçu technique** : première version publiée, pour essayer le principe de Prebench. Elle simule une seule carte, la Nucleo-F401RE, et seulement ses GPIO.

### Ajouté

- Simulation d'un projet STM32 HAL sur Nucleo-F401RE, sans modifier le code : un projet généré par STM32CubeMX se compile tel quel pour l'ordinateur et s'exécute dans Prebench.
- GPIO, interruptions EXTI, `HAL_Delay` et `HAL_GetTick` simulés. La LED LD2 et le bouton B1 de la carte sont intégrés à la paillasse.
- UART, TIM, ADC, I2C, SPI et DMA : le code compile, et Prebench affiche « non simulé » dans l'onglet ⚠ ; leur simulation arrive dans les versions suivantes.
- Composants LED, bouton poussoir et écho (mesure de latence).
- Projet de départ « blinky STM32 HAL » : LD2 clignote, B1 change la vitesse.
- Onglet Journal (sorties du firmware) et onglet ⚠ (avertissements).
- Firmware exécuté en priorité basse, pour ne pas ralentir l'ordinateur ; la priorité effective est affichée dans le Journal.
- Mises à jour automatiques, vérifiées par signature.

### Limites connues

- La boucle principale vide d'un projet CubeMX (`while (1) {}`) occupe un cœur à 100 % ; le plafond CPU avec pause automatique arrive dans une version suivante.
- Pas encore de câblage dans l'interface : les composants se déclarent dans `prebench.toml`.
- Le projet est compilé quand on clique **▶ Démarrer** ; la recompilation à chaque sauvegarde arrive dans une version suivante.
- L'app n'est pas signée : voir la procédure de premier lancement dans le README (macOS et Windows).
