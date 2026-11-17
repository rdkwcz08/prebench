/**
 * components/component-api.d.ts — Interface des composants Prebench, version 1.
 *
 * Un composant est un dossier components/<id>/ contenant :
 *   - manifest.json : description déclarative (schéma : manifest.schema.json) ;
 *   - model.ts      : comportement, exporte par défaut une ComponentFactory ;
 *   - view.svelte   : rendu, reçoit des ComponentViewProps.
 *
 * Le modèle s'exécute dans QuickJS, au cœur de Prebench (un runtime par
 * instance) : aucun accès au disque, au réseau ni au système (règle 16.2.8).
 * Limites : 16 Mo, pile 256 Ko, 50 ms au chargement ; par appel, budget de 5 ms de
 * temps CPU (un dépassement isolé est toléré, trois de suite arrêtent le composant)
 * et plafond dur de 50 ms (boucle infinie : arrêt immédiat). Il ne voit que les événements qui concernent
 * ses broches et ses bus. Toutes les méthodes sont synchrones.
 *
 * Évolution : les ajouts compatibles (nouvelle méthode optionnelle, nouveau
 * champ) gardent la version 1 ; toute rupture crée la version 2, déclarée dans
 * le champ `api_version` du manifeste.
 *
 * Références : cahier des charges sections 8 et 9, docs/protocol.md.
 */

/** Version de l'interface décrite par ce fichier. */
export type ComponentApiVersion = 1;

/* ------------------------------------------------------------------------- */
/* Valeurs                                                                   */
/* ------------------------------------------------------------------------- */

/** Valeur scalaire d'un paramètre ou d'une cellule de table. */
export type ScalarParamValue = number | string | boolean;

/**
 * Valeur d'un paramètre de type « table » : une ligne par entrée, une clé par
 * colonne déclarée dans le manifeste (ex. table de registres du joker I2C).
 * Ajout compatible de la v1 (phase 3).
 */
export type TableParamValue = ReadonlyArray<Readonly<Record<string, ScalarParamValue>>>;

/** Valeur d'un paramètre déclaré dans le manifeste. */
export type ParamValue = ScalarParamValue | TableParamValue;

/** Valeur transmise par un contrôle (bouton, interrupteur, curseur). */
export type ControlValue = number | string | boolean;

/**
 * État logique d'un net (section 9.1).
 * - 'high' / 'low' : une sortie (ou une alimentation) impose le niveau ;
 * - 'pulled-high' / 'pulled-low' : seule une résistance de tirage agit ;
 * - 'floating' : rien ne fixe le niveau ;
 * - 'conflict' : deux sorties imposent des niveaux opposés.
 */
export type NetLevel = 'high' | 'low' | 'pulled-high' | 'pulled-low' | 'floating' | 'conflict';

/** Signal PWM présent sur un net : on simule le rapport cyclique, pas le signal. */
export interface PwmInfo {
  /** Fréquence en Hz. */
  readonly freqHz: number;
  /** Rapport cyclique de 0 (toujours bas) à 1 (toujours haut). */
  readonly duty: number;
}

/** État d'un net vu depuis une broche du composant. */
export interface PinState {
  /**
   * Niveau logique. Si `pwm` est présent : 'low' pour duty = 0, sinon 'high'.
   * Les modèles qui gèrent la PWM utilisent `pwm.duty`.
   */
  readonly level: NetLevel;
  /** Présent uniquement si le net est piloté par une sortie PWM active. */
  readonly pwm?: PwmInfo;
  /**
   * Tension du net en volts, quand elle est connue : tension fixée par un
   * composant (setVoltage), sinon alimentation de la carte (3V3, 5V, GND).
   * Absente pour un net purement logique. Ajout compatible de la v1 (étape 8).
   */
  readonly volts?: number;
}

/** Niveau imposé par le composant sur l'une de ses broches. 'z' = haute impédance. */
export type DriveLevel = 'high' | 'low' | 'z';

/** Résistance de tirage apportée par le composant sur l'une de ses broches. */
export type PullKind = 'up' | 'down' | 'none';

/** Vecteur 3 axes. */
export interface Vec3 {
  readonly x: number;
  readonly y: number;
  readonly z: number;
}

/** Grandeurs physiques du panneau « environnement » (section 8.5). */
export interface Environment {
  readonly temperatureC: number;
  readonly humidityPercent: number;
  readonly illuminanceLux: number;
  /** Inclinaison en degrés. */
  readonly tiltDeg: { readonly roll: number; readonly pitch: number };
  /** Accélération en g, gravité comprise. */
  readonly accelerationG: Vec3;
  readonly distanceM: number;
}

/* ------------------------------------------------------------------------- */
/* Bus                                                                       */
/* ------------------------------------------------------------------------- */

/** Transaction I2C maître adressée au composant (docs/protocol.md, I2C_XFER). */
export interface I2CRequest {
  /** Adresse 7 bits. */
  readonly address: number;
  /** Octets écrits par le firmware (peut être vide). */
  readonly tx: Uint8Array;
  /** Nombre d'octets que le firmware lit ensuite (0 = écriture seule). */
  readonly rxLength: number;
}

/**
 * Réponse I2C. Avec ack: true, `rx` DOIT contenir exactement rxLength octets
 * (complétés par 0xFF par l'hôte sinon, avec un avertissement).
 */
export type I2CResponse =
  | { readonly ack: true; readonly rx?: Uint8Array }
  | { readonly ack: false; readonly nack: 'address' | 'data' };

/** Transaction SPI vue par le composant sélectionné. */
export interface SPIRequest {
  /** Octets émis par le firmware (0xFF si le firmware ne fait que lire). */
  readonly tx: Uint8Array;
  /** true si le firmware lit la réponse (transaction synchrone). */
  readonly wantsRx: boolean;
}

/* ------------------------------------------------------------------------- */
/* Écrans — ajout compatible de la v1 (phase 3)                              */
/* ------------------------------------------------------------------------- */

/**
 * Format des pixels d'un écran, ordre exact des octets et des bits :
 * - 'rgb565' : 2 octets par pixel, octet de poids fort d'abord (ordre du fil
 *   d'un ILI9341 ou d'un ST7735) ; pixels de gauche à droite, lignes de haut
 *   en bas. Ex. : [0xF8, 0x00] = rouge pur (0xF800), [0x00, 0x1F] = bleu pur.
 * - 'mono1' : 1 bit par pixel, lignes horizontales de haut en bas ; chaque
 *   ligne commence sur un nouvel octet (⌈largeur ÷ 8⌉ octets par ligne) ; bit 7
 *   (poids fort) = pixel de gauche de l'octet ; 1 = allumé.
 *   Ex. : 0b1000_0001 = pixels 0 et 7 de la ligne allumés.
 * - 'mono1-vertical' : 1 bit par pixel, pages de 8 lignes de haut en bas ;
 *   dans une page, un octet par colonne, de gauche à droite ; bit 0 (poids
 *   faible) = ligne du haut de la page ; 1 = allumé (mémoire d'un SSD1306,
 *   SH1106, PCD8544). Ex. : en page 1, 0b1000_0001 = lignes 8 et 15 allumées.
 * Référence et tests : app/src-tauri/src/core/components/display.rs.
 */
export type DisplayFormat = 'mono1-vertical' | 'mono1' | 'rgb565';

export interface DisplayOptions {
  /** Largeur en pixels, de 1 à 1024. */
  readonly width: number;
  /** Hauteur en pixels, de 1 à 1024 (multiple de 8 en 'mono1-vertical'). */
  readonly height: number;
  readonly format: DisplayFormat;
}

/**
 * Image d'un écran. Les pixels vivent dans le cœur, hors de QuickJS et de sa
 * limite de 16 Mo : le modèle décode les commandes de l'écran et copie les
 * pixels par blit, sans boucle JavaScript sur l'image. Le cœur accumule la
 * zone modifiée et n'envoie à la vue que cette zone, au plus une fois par
 * image, et seulement si la fenêtre est visible (section 11.3). L'image est
 * noire (tout éteint) à la création.
 */
export interface Display {
  readonly width: number;
  readonly height: number;
  readonly format: DisplayFormat;
  /**
   * Copie un rectangle de pixels, ligne par ligne, au format de l'écran.
   * `data` contient exactement les octets du rectangle : w × h × 2 en 'rgb565',
   * ⌈w ÷ 8⌉ × h en 'mono1' (chaque ligne commence sur un octet), w × h ÷ 8 en
   * 'mono1-vertical' (y et h multiples de 8, pages de haut en bas, colonnes de
   * gauche à droite). Un rectangle qui déborde de l'écran est rogné ; des
   * données trop courtes lèvent une erreur.
   */
  blit(x: number, y: number, w: number, h: number, data: Uint8Array): void;
  /**
   * Ordre de balayage des blits suivants, comme le registre MADCTL d'un
   * ILI9341 ou d'un ST7735 ('rgb565' seulement ; erreur sinon, sauf l'ordre
   * par défaut). Les coordonnées des blits sont alors celles de l'image
   * logique, et le cœur place chaque pixel dans l'image physique :
   * (a, b) = swapXY ? (y, x) : (x, y) ; x physique = mirrorX ? width − 1 − a : a ;
   * y physique = mirrorY ? height − 1 − b : b. Avec swapXY, l'image logique
   * mesure height × width. bgr : rouge et bleu échangés dans chaque pixel.
   * Le cœur fait ce travail : en JavaScript, tourner un écran entier coûterait
   * l'essentiel du budget d'un appel. Ajout compatible de la v1 (phase 3).
   */
  setScan(scan: DisplayScan): void;
}

/** Ordre de balayage d'un écran (Display.setScan) ; champs absents : false. */
export interface DisplayScan {
  readonly swapXY?: boolean;
  readonly mirrorX?: boolean;
  readonly mirrorY?: boolean;
  readonly bgr?: boolean;
}

/* ------------------------------------------------------------------------- */
/* Contexte fourni au modèle                                                 */
/* ------------------------------------------------------------------------- */

/** Services offerts par l'hôte au modèle. Seul canal de sortie du modèle. */
export interface ComponentContext<P extends object> {
  readonly apiVersion: ComponentApiVersion;
  /** Identifiant d'instance dans prebench.toml (ex. "led1") ou de la carte (ex. "LD2"). */
  readonly id: string;
  /** Paramètres courants, validés contre le manifeste ; suivent onParamsChange. */
  readonly params: Readonly<P>;
  /** Tension logique de la carte, en volts (3,3 pour la Nucleo). */
  readonly logicVoltage: number;

  /** État courant du net relié à une broche du composant. */
  pin(name: string): PinState;
  /** Impose un niveau sur une broche déclarée en sortie ou entrée/sortie. */
  drive(pin: string, level: DriveLevel): void;
  /** Ajoute ou retire une résistance de tirage sur une broche. */
  pull(pin: string, kind: PullKind): void;
  /**
   * Fixe la tension d'une broche analogique (lue par l'ADC), en volts. Une
   * valeur non finie (NaN) retire la tension : le net redevient logique.
   */
  setVoltage(pin: string, volts: number): void;
  /** Émet des octets sur l'interface UART `iface` du manifeste (défaut : la première). */
  uartWrite(bytes: Uint8Array, iface?: string): void;
  /** Signale que l'état a changé : la vue sera redessinée à la prochaine image. */
  invalidate(): void;
  /**
   * Crée l'image de l'écran du composant (un seul écran par composant). Un
   * nouvel appel remplace l'écran précédent, par exemple quand un paramètre
   * change la résolution. Ajout compatible de la v1 (phase 3).
   */
  createDisplay(options: DisplayOptions): Display;
  /** Avertissement affiché dans l'onglet ⚠. */
  warn(message: string): void;
  /** Ligne de journal (débogage du composant). */
  log(message: string): void;
  /** Temps de simulation en µs (horodatage du dernier événement reçu). */
  now(): number;
}

/* ------------------------------------------------------------------------- */
/* Modèle                                                                    */
/* ------------------------------------------------------------------------- */

/**
 * Comportement d'un composant. Toutes les méthodes sont optionnelles sauf
 * getState. L'état renvoyé DOIT être clonable (structured clone) : il est
 * transmis à la vue (JSON), au plus une fois par image.
 */
export interface ComponentModel<S> {
  /** Changement d'état du net relié à une broche. */
  onPinChange?(pin: string, state: PinState): void;
  /** Transaction I2C adressée à ce composant. */
  onI2C?(request: I2CRequest): I2CResponse;
  /** Transaction SPI pendant que ce composant est sélectionné. Renvoie tx.length octets. */
  onSPI?(request: SPIRequest): Uint8Array | void;
  /** Octets reçus sur une interface UART du composant. */
  onUART?(bytes: Uint8Array, iface: string): void;
  /**
   * Trame entière reçue sur l'entrée de données d'une interface « led-strip »
   * (docs/protocol.md, LED_STRIP), octets dans l'ordre du fil (GRB pour un
   * WS2812). Ajout compatible de la v1 (phase 3).
   */
  onLedStrip?(data: Uint8Array, iface: string): void;
  /** Appelé `tick_hz` fois par seconde si le manifeste le demande (0 = jamais). */
  onTick?(dtMs: number): void;
  /** Action de l'utilisateur sur un contrôle déclaré dans le manifeste. */
  onControl?(control: string, value: ControlValue): void;
  /** Valeurs du panneau environnement (au démarrage puis à chaque changement). */
  onEnvironment?(env: Environment): void;
  /** Paramètres modifiés dans l'inspecteur. */
  onParamsChange?(params: Readonly<Record<string, ParamValue>>): void;
  /** Alimentation de la paillasse coupée (false) ou rétablie (true). */
  onPower?(on: boolean): void;
  /**
   * Diagnostic : requête PING du firmware (vmcu_ping, cible VMCU_PING_WORKER). Le retour
   * de la méthode déclenche la réponse PONG. Implémentée par le composant
   * d'écho (components/echo/) pour mesurer la latence de la section 8.1.
   */
  onPing?(): void;
  /** État à dessiner ; appelé au plus une fois par image, après invalidate(). */
  getState(): S;
}

/** Export par défaut de model.ts. */
export type ComponentFactory<P extends object, S> = (ctx: ComponentContext<P>) => ComponentModel<S>;

/* ------------------------------------------------------------------------- */
/* Vue                                                                       */
/* ------------------------------------------------------------------------- */

/** Couleurs et options de rendu d'un écran, choisies par la vue. */
export interface DisplaySurfaceOptions {
  /** Couleur CSS d'un pixel allumé (écrans monochromes). */
  readonly on?: string;
  /** Couleur CSS d'un pixel éteint (écrans monochromes). */
  readonly off?: string;
  /**
   * Inverse les pixels (commande d'inversion d'un SSD1306) ; en 'rgb565',
   * chaque composante est complémentée (INVON d'un ILI9341).
   */
  readonly invert?: boolean;
}

/**
 * Élément où l'hôte dessine : un HTMLCanvasElement, décrit ici sans dépendre
 * des types du DOM (les modèles sont vérifiés sans eux).
 */
export interface DisplayCanvas {
  width: number;
  height: number;
}

/**
 * Écran vu par la vue : l'hôte dessine lui-même dans le canvas attaché, zone
 * modifiée seulement, sans repasser par Svelte. `attach` s'utilise comme une
 * action Svelte : `<canvas use:display.attach={{ on, off, invert }} />`. L'hôte
 * fixe la taille du canvas en pixels de l'écran ; la vue le met à l'échelle en
 * CSS (`image-rendering: pixelated`).
 */
export interface DisplaySurface {
  readonly width: number;
  readonly height: number;
  readonly format: DisplayFormat;
  readonly attach: (
    canvas: DisplayCanvas,
    options?: DisplaySurfaceOptions,
  ) => { update(options?: DisplaySurfaceOptions): void; destroy(): void };
}

/** Propriétés reçues par view.svelte. */
export interface ComponentViewProps<P extends object, S> {
  /** Dernier état renvoyé par getState. */
  readonly state: S;
  readonly params: Readonly<P>;
  /** 'edit' : placer et câbler ; 'simulate' : interagir. */
  readonly mode: 'edit' | 'simulate';
  /** Transmet une action utilisateur au modèle (onControl). */
  control(id: string, value: ControlValue): void;
  /**
   * Écran du composant, présent quand le modèle a appelé createDisplay.
   * Ajout compatible de la v1 (phase 3).
   */
  readonly display?: DisplaySurface;
}
