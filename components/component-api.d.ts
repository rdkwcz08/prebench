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
 * Limites : 16 Mo, pile 256 Ko, 5 ms par appel, 50 ms au chargement. Il ne voit que les événements qui concernent
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

/** Valeur d'un paramètre déclaré dans le manifeste. */
export type ParamValue = number | string | boolean;

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
/* Contexte fourni au modèle                                                 */
/* ------------------------------------------------------------------------- */

/** Services offerts par l'hôte au modèle. Seul canal de sortie du modèle. */
export interface ComponentContext<P extends object> {
  readonly apiVersion: ComponentApiVersion;
  /** Identifiant d'instance dans prebench.toml (ex. "led1") ou de la carte (ex. "LD2"). */
  readonly id: string;
  /** Paramètres courants, validés contre le manifeste. */
  readonly params: Readonly<P>;
  /** Tension logique de la carte, en volts (3,3 pour la Nucleo). */
  readonly logicVoltage: number;

  /** État courant du net relié à une broche du composant. */
  pin(name: string): PinState;
  /** Impose un niveau sur une broche déclarée en sortie ou entrée/sortie. */
  drive(pin: string, level: DriveLevel): void;
  /** Ajoute ou retire une résistance de tirage sur une broche. */
  pull(pin: string, kind: PullKind): void;
  /** Fixe la tension d'une broche analogique (lue par l'ADC), en volts. */
  setVoltage(pin: string, volts: number): void;
  /** Émet des octets sur l'interface UART `iface` du manifeste (défaut : la première). */
  uartWrite(bytes: Uint8Array, iface?: string): void;
  /** Signale que l'état a changé : la vue sera redessinée à la prochaine image. */
  invalidate(): void;
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

/** Propriétés reçues par view.svelte. */
export interface ComponentViewProps<P extends object, S> {
  /** Dernier état renvoyé par getState. */
  readonly state: S;
  readonly params: Readonly<P>;
  /** 'edit' : placer et câbler ; 'simulate' : interagir. */
  readonly mode: 'edit' | 'simulate';
  /** Transmet une action utilisateur au modèle (onControl). */
  control(id: string, value: ControlValue): void;
}
