/**
 * TypeScript definitions for BitCraps WebAssembly module
 * Auto-generated bindings for browser integration
 */

declare module 'bitcraps-wasm' {
  /**
   * Main BitCraps WASM runtime interface
   */
  export class BitCrapsWasm {
    /**
     * Create a new BitCraps instance
     * @param config Optional browser configuration
     */
    constructor(config?: BrowserConfig);

    /**
     * Get the peer ID for this browser instance
     */
    readonly peer_id: string;

    /**
     * Initialize the runtime and start services
     */
    initialize(): Promise<string>;

    /**
     * Create a new craps game
     * @param gameId Unique game identifier
     */
    create_game(gameId: string): Promise<string>;

    /**
     * Join an existing game
     * @param gameId Game identifier to join
     */
    join_game(gameId: string): Promise<string>;

    /**
     * Execute a game action
     * @param gameId Game identifier
     * @param action Game action to execute
     */
    execute_action(gameId: string, action: JsGameAction): Promise<JsGameState>;

    /**
     * Get current game state
     * @param gameId Game identifier
     */
    get_game_state(gameId: string): Promise<JsGameState>;

    /**
     * Get list of available games
     */
    list_games(): Promise<string[]>;

    /**
     * Connect to peers via WebRTC
     * @param signalingServer Optional signaling server URL
     */
    connect_peers(signalingServer?: string): Promise<string>;

    /**
     * Get list of connected peers
     */
    get_peers(): Promise<JsPeerInfo[]>;

    /**
     * Send message to a peer
     * @param peerId Target peer ID
     * @param message Message bytes
     */
    send_message(peerId: string, message: Uint8Array): Promise<boolean>;

    /**
     * Load a WASM plugin
     * @param name Plugin name
     * @param wasmBytes Plugin WASM bytecode
     */
    load_plugin(name: string, wasmBytes: Uint8Array): Promise<string>;

    /**
     * Get runtime statistics
     */
    get_stats(): Promise<RuntimeStats>;

    /**
     * Enable debug logging
     */
    enable_debug(): void;

    /**
     * Get browser information
     */
    get_browser_info(): BrowserInfo;
  }

  /**
   * Game state representation for JavaScript
   */
  export class JsGameState {
    constructor(gameId: string, phase: string);
    
    readonly game_id: string;
    readonly phase: string;
    readonly point: number | null;
    readonly players: any[];
    readonly bets: Record<string, any>;
  }

  /**
   * Game action representation for JavaScript
   */
  export class JsGameAction {
    constructor(actionType: string, playerId: string);
    
    readonly action_type: string;
    readonly player_id: string;
    
    set_amount(amount: number): void;
    set_bet_type(betType: string): void;
  }

  /**
   * Browser configuration options
   */
  export interface BrowserConfig {
    enable_webrtc?: boolean;
    signaling_server?: string;
    stun_servers?: string[];
    max_peers?: number;
    auto_connect?: boolean;
    debug_mode?: boolean;
  }

  /**
   * Peer information
   */
  export interface JsPeerInfo {
    peer_id: string;
    connected: boolean;
    transport_type: string;
    connection_time?: number;
    last_activity?: number;
  }

  /**
   * Runtime statistics
   */
  export interface RuntimeStats {
    modules_loaded: number;
    active_instances: number;
    total_executions: number;
    memory_usage: number;
  }

  /**
   * Browser information
   */
  export interface BrowserInfo {
    user_agent?: string;
    platform?: string;
    language?: string;
    origin?: string;
    protocol?: string;
    wasm_supported: boolean;
    webrtc_supported: boolean;
  }

  /**
   * Game phases
   */
  export enum GamePhase {
    ComeOut = "come_out",
    Point = "point",
    GameOver = "game_over",
  }

  /**
   * Bet types for craps game
   */
  export enum BetType {
    PassLine = "pass_line",
    DontPass = "dont_pass",
    Come = "come",
    DontCome = "dont_come",
    Field = "field",
    Place = "place",
    Buy = "buy",
    Lay = "lay",
    Hardways = "hardways",
    OneRoll = "one_roll",
  }

  /**
   * Action types
   */
  export enum ActionType {
    PlaceBet = "place_bet",
    RollDice = "roll_dice",
    JoinGame = "join_game",
    LeaveGame = "leave_game",
    StartGame = "start_game",
  }

  /**
   * Transport types
   */
  export enum TransportType {
    WebRTC = "webrtc",
    WebSocket = "websocket",
    HTTP = "http",
  }

  /**
   * Error types that can be thrown by the WASM module
   */
  export class WasmError extends Error {
    constructor(message: string);
  }

  /**
   * Initialize the BitCraps WASM module
   */
  export function initialize_bitcraps_wasm(): void;

  /**
   * Get WASM module version
   */
  export function get_version(): string;

  /**
   * Access WASM memory (for advanced users)
   */
  export function wasm_memory(): WebAssembly.Memory;

  // Utility types for TypeScript development
  
  /**
   * Event handler type for game events
   */
  export type GameEventHandler = (gameId: string, event: GameEvent) => void;

  /**
   * Event handler type for network events
   */
  export type NetworkEventHandler = (peerId: string, event: NetworkEvent) => void;

  /**
   * Game events
   */
  export interface GameEvent {
    type: string;
    timestamp: number;
    data?: any;
  }

  /**
   * Network events
   */
  export interface NetworkEvent {
    type: string;
    timestamp: number;
    data?: any;
  }

  /**
   * Plugin interface for custom game logic
   */
  export interface GamePlugin {
    name: string;
    version: string;
    description: string;
    initialize(): Promise<void>;
    execute(action: JsGameAction, state: JsGameState): Promise<JsGameState>;
    cleanup(): Promise<void>;
  }

  /**
   * WebRTC configuration options
   */
  export interface WebRTCConfig {
    iceServers: RTCIceServer[];
    iceTransportPolicy?: RTCIceTransportPolicy;
    bundlePolicy?: RTCBundlePolicy;
    rtcpMuxPolicy?: RTCRtcpMuxPolicy;
  }

  /**
   * Signaling message types for WebRTC
   */
  export interface SignalingMessage {
    type: 'offer' | 'answer' | 'ice-candidate' | 'discover' | 'announce';
    peer_id: string;
    data?: any;
  }

  /**
   * Data channel configuration
   */
  export interface DataChannelConfig {
    label: string;
    ordered?: boolean;
    maxPacketLifeTime?: number;
    maxRetransmits?: number;
    protocol?: string;
  }

  /**
   * Connection statistics
   */
  export interface ConnectionStats {
    bytesReceived: number;
    bytesSent: number;
    packetsReceived: number;
    packetsSent: number;
    connectTime: number;
    lastActivity: number;
  }

  /**
   * Memory usage information
   */
  export interface MemoryUsage {
    totalAllocated: number;
    peakAllocation: number;
    currentInstances: number;
    gcRuns: number;
  }

  /**
   * Performance metrics
   */
  export interface PerformanceMetrics {
    averageExecutionTime: number;
    totalExecutions: number;
    successRate: number;
    errorRate: number;
    memoryEfficiency: number;
  }

  /**
   * Promise-based API for easier async/await usage
   */
  export namespace API {
    /**
     * Create and initialize a BitCraps instance
     */
    export function create(config?: BrowserConfig): Promise<BitCrapsWasm>;

    /**
     * Connect to the BitCraps network
     */
    export function connect(
      instance: BitCrapsWasm,
      options?: {
        signalingServer?: string;
        autoConnect?: boolean;
        maxRetries?: number;
      }
    ): Promise<void>;

    /**
     * Create or join a game
     */
    export function joinGame(
      instance: BitCrapsWasm,
      gameId?: string
    ): Promise<{
      gameId: string;
      isCreator: boolean;
      initialState: JsGameState;
    }>;

    /**
     * Place a bet in the game
     */
    export function placeBet(
      instance: BitCrapsWasm,
      gameId: string,
      betType: BetType,
      amount: number
    ): Promise<JsGameState>;

    /**
     * Roll the dice (if it's your turn)
     */
    export function rollDice(
      instance: BitCrapsWasm,
      gameId: string
    ): Promise<{
      result: [number, number];
      newState: JsGameState;
    }>;

    /**
     * Get game history
     */
    export function getGameHistory(
      instance: BitCrapsWasm,
      gameId: string
    ): Promise<GameEvent[]>;

    /**
     * Get player statistics
     */
    export function getPlayerStats(
      instance: BitCrapsWasm,
      playerId?: string
    ): Promise<{
      gamesPlayed: number;
      totalWinnings: number;
      winRate: number;
      favoriteGame: string;
    }>;
  }

  /**
   * Event emitter interface for game and network events
   */
  export interface EventEmitter {
    on(event: string, handler: (...args: any[]) => void): void;
    off(event: string, handler: (...args: any[]) => void): void;
    emit(event: string, ...args: any[]): void;
  }

  /**
   * Global constants
   */
  export const CONSTANTS: {
    readonly MAX_PLAYERS: number;
    readonly MIN_BET: number;
    readonly MAX_BET: number;
    readonly DICE_SIDES: number;
    readonly GAME_TIMEOUT: number;
    readonly CONNECTION_TIMEOUT: number;
    readonly HEARTBEAT_INTERVAL: number;
  };
}