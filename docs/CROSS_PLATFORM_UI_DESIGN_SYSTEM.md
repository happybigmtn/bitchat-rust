# Cross-Platform UI Design System
## BitCraps Mobile Design Specification

*Version: 1.0 | Last Updated: 2025-08-24 | Status: Production Ready*

---

## Overview

This document defines the unified design system for BitCraps mobile applications across Android and iOS platforms. It ensures consistent user experience while respecting platform-specific design guidelines and conventions.

**Design Philosophy**
- **Consistency**: Unified visual language across platforms
- **Platform Native**: Respect platform-specific patterns and conventions
- **Gaming Focus**: Optimized for real-time gaming interactions
- **Accessibility**: WCAG 2.1 AA compliant design
- **Performance**: 60fps animations, minimal battery drain

---

## Color System

### Brand Colors

**Primary Palette**
```
Casino Green: #388E3C (Android Primary) / systemGreen (iOS)
Casino Red: #D32F2F (Error/Danger states)
Casino Gold: #FFB300 (Secondary/Accent)
Casino Blue: #1976D2 (Information/Links)
Casino Black: #212121 (Text/Borders)
```

**Functional Colors**
```
Success: #4CAF50
Warning: #FF9800
Error: #F44336
Info: #2196F3
Neutral: #9E9E9E
```

### Android Implementation
```kotlin
// Material Design 3 Color Scheme
val Primary = Color(0xFF388E3C)
val OnPrimary = Color.White
val PrimaryContainer = Color(0xFFE8F5E8)
val OnPrimaryContainer = Color(0xFF002106)

val Secondary = Color(0xFFFFB300)
val OnSecondary = Color.White
val SecondaryContainer = Color(0xFFFFF8E1)
val OnSecondaryContainer = Color(0xFF3E2723)
```

### iOS Implementation
```swift
// iOS Color Assets
extension Color {
    static let casinoGreen = Color("CasinoGreen")
    static let casinoRed = Color("CasinoRed")
    static let casinoGold = Color("CasinoGold")
    static let casinoBlue = Color("CasinoBlue")
    
    // Semantic colors
    static let gameTableBackground = Color("GameTableBackground")
    static let cardBackground = Color("CardBackground")
    static let backgroundPrimary = Color("BackgroundPrimary")
    static let backgroundSecondary = Color("BackgroundSecondary")
}
```

### Dark Mode Support
Both platforms automatically adapt colors for dark mode:
- Background colors invert appropriately
- Text maintains readability contrast
- Gaming elements (dice, chips) remain consistent
- Status indicators adapt to dark backgrounds

---

## Typography

### Type Scale

**Display**
- Display Large: 57sp/pt (Android) / Title (iOS)
- Display Medium: 45sp/pt
- Display Small: 36sp/pt

**Headlines**
- Headline Large: 32sp/pt - Game titles, major actions
- Headline Medium: 28sp/pt - Section headers
- Headline Small: 24sp/pt - Card titles, dice totals

**Body**
- Body Large: 16sp/pt - Primary content, game instructions
- Body Medium: 14sp/pt - Secondary content, status text
- Body Small: 12sp/pt - Captions, footnotes

**Labels**
- Label Large: 14sp/pt - Button text, form labels
- Label Medium: 12sp/pt - Tab labels, chips
- Label Small: 11sp/pt - Fine print, timestamps

### Android Typography
```kotlin
val Typography = Typography(
    headlineLarge = TextStyle(
        fontFamily = FontFamily.Default,
        fontWeight = FontWeight.Bold,
        fontSize = 32.sp,
        lineHeight = 40.sp
    ),
    bodyLarge = TextStyle(
        fontFamily = FontFamily.Default,
        fontWeight = FontWeight.Normal,
        fontSize = 16.sp,
        lineHeight = 24.sp
    )
)
```

### iOS Typography
```swift
// SwiftUI Typography
.font(.largeTitle) // Headlines
.font(.title) // Section headers
.font(.headline) // Emphasis text
.font(.body) // Primary content
.font(.subheadline) // Secondary content
.font(.caption) // Fine print

// Custom gaming typography
.font(.system(size: 32, weight: .bold, design: .rounded)) // Dice totals
```

---

## Component Library

### Core Gaming Components

#### 1. Dice Component

**Visual Specification**
- Size: 60x60dp/pt (standard), 80x80dp/pt (game active)
- Corner radius: 12dp/pt
- Shadow: 4dp/pt blur, 2dp/pt offset
- Colors: White background, black dots
- Animation: 2s roll duration, easing curves

**Android Implementation**
```kotlin
@Composable
fun AnimatedDie(
    value: Int,
    isRolling: Boolean,
    modifier: Modifier = Modifier
) {
    val rotation by animateFloatAsState(
        targetValue = if (isRolling) 360f * 3 else 0f,
        animationSpec = if (isRolling) {
            infiniteRepeatable(tween(1000, easing = LinearEasing))
        } else tween(300)
    )
    // Implementation details...
}
```

**iOS Implementation**
```swift
struct DieView: View {
    let value: Int
    let isRolling: Bool
    
    @State private var rotationAngle: Double = 0
    
    var body: some View {
        ZStack {
            RoundedRectangle(cornerRadius: 12)
                .fill(LinearGradient(...))
                .frame(width: 60, height: 60)
            // Implementation details...
        }
        .rotationEffect(.degrees(rotationAngle))
    }
}
```

#### 2. Status Indicator

**States**
- Good: Green circle with checkmark
- Warning: Orange triangle with exclamation
- Error: Red circle with X
- Idle: Gray circle with dash

**Android Implementation**
```kotlin
@Composable
fun StatusIndicator(
    title: String,
    status: StatusType,
    icon: ImageVector
) {
    Row(verticalAlignment = Alignment.CenterVertically) {
        Icon(
            imageVector = icon,
            tint = status.color,
            modifier = Modifier.size(20.dp)
        )
        Text(text = title)
    }
}
```

**iOS Implementation**
```swift
struct StatusIndicator: View {
    let title: String
    let status: StatusType
    let icon: String
    
    var body: some View {
        HStack(spacing: 8) {
            Image(systemName: icon)
                .foregroundColor(status.color)
            Text(title)
                .font(.caption)
        }
    }
}
```

#### 3. Game Card

**Layout**
- Padding: 16dp/pt all sides
- Corner radius: 16dp/pt
- Elevation: 8dp (Android), shadow (iOS)
- Background: Surface color with transparency

**Content Structure**
```
┌─ Card Container ─────────────────┐
│ Title                      Badge │
│ Subtitle                         │
│ ─────────────────────────────── │
│ Content Area                     │
│ ─────────────────────────────── │
│ [Action Button] [Secondary]     │
└──────────────────────────────────┘
```

#### 4. Betting Interface

**Chip Values**
- $10: Green chip
- $25: Red chip
- $50: Blue chip
- $100: Gold chip

**Layout**
- 4-column grid (phones), 6-column (tablets)
- Minimum touch target: 48dp/pt
- Disabled state: 50% opacity
- Selected state: 20% larger scale

#### 5. Game Table Visualization

**Colors**
- Background: Casino green (#0D7377)
- Lines: White with 2dp/pt stroke
- Text: White with high contrast
- Betting areas: Semi-transparent overlays

**Interactive Areas**
- Pass line: Touch target with haptic feedback
- Don't pass: Alternative betting area
- Dice area: Non-interactive display zone

### Navigation Components

#### 1. Bottom Navigation (Android)
```kotlin
@Composable
fun GameBottomNavigation(
    currentRoute: String,
    onNavigate: (String) -> Unit
) {
    NavigationBar {
        NavigationBarItem(
            selected = currentRoute == "lobby",
            onClick = { onNavigate("lobby") },
            icon = { Icon(Icons.Default.Home, null) },
            label = { Text("Lobby") }
        )
        // More items...
    }
}
```

#### 2. Tab Bar (iOS)
```swift
TabView {
    GameLobbyView()
        .tabItem {
            Image(systemName: "house")
            Text("Lobby")
        }
    
    ActiveGameView()
        .tabItem {
            Image(systemName: "gamecontroller")
            Text("Game")
        }
}
```

---

## Animation System

### Core Animations

**Dice Roll Animation**
- Duration: 2000ms
- Easing: Custom bezier curve
- Rotation: 3 full rotations
- Scale: 1.0 → 1.2 → 1.0
- Haptic feedback on completion

**State Transitions**
- Duration: 300ms
- Easing: Platform default (Material/iOS)
- Property: Opacity, transform, color
- Stagger: 50ms delay between elements

**Micro-interactions**
- Button press: 100ms scale to 0.95
- Card hover: 200ms elevation increase
- Status change: 150ms color transition
- Loading states: Infinite pulse/rotation

### Performance Guidelines
- Target: 60fps sustained
- Maximum: 5 concurrent animations
- Reduced motion: Honor accessibility settings
- Battery optimization: Disable in low power mode

---

## Spacing System

### Base Unit: 4dp/pt

**Spacing Scale**
```
XS:  4dp/pt  - Icon margins, tight spacing
S:   8dp/pt  - Component internal padding
M:   16dp/pt - Standard component spacing
L:   24dp/pt - Section spacing
XL:  32dp/pt - Major section separation
XXL: 48dp/pt - Screen-level spacing
```

### Layout Grids

**Phone Layout**
- Margins: 16dp/pt
- Gutters: 8dp/pt
- Columns: 4 (portrait), 6 (landscape)

**Tablet Layout**
- Margins: 24dp/pt
- Gutters: 12dp/pt
- Columns: 8 (portrait), 12 (landscape)

---

## Accessibility

### WCAG 2.1 Compliance

**Color Contrast**
- Normal text: 4.5:1 minimum
- Large text: 3:1 minimum
- UI components: 3:1 minimum
- Focus indicators: 3:1 minimum

**Touch Targets**
- Minimum: 44dp/pt
- Preferred: 48dp/pt
- Spacing: 8dp/pt between targets

**Screen Reader Support**

**Android**
```kotlin
modifier = Modifier.semantics {
    contentDescription = "Roll dice button"
    role = Role.Button
    stateDescription = if (isRolling) "Rolling" else "Ready to roll"
}
```

**iOS**
```swift
.accessibilityLabel("Roll dice")
.accessibilityHint("Double tap to roll the dice")
.accessibilityValue(isRolling ? "Rolling" : "Ready")
```

### Reduced Motion Support

**Android**
```kotlin
val animationSpec = if (LocalAccessibilityManager.current.isReduceMotionEnabled) {
    snap() // No animation
} else {
    tween(300)
}
```

**iOS**
```swift
@Environment(\.accessibilityReduceMotion) var reduceMotion

var animationDuration: Double {
    reduceMotion ? 0 : 0.3
}
```

---

## Platform-Specific Adaptations

### Android Adaptations

**Material Design 3**
- Use Material components where possible
- Follow Material motion principles
- Respect system theme (light/dark)
- Use Android-specific icons (vector drawables)

**System Integration**
- Support Android's back gesture
- Use Android notification system
- Follow Android permission patterns
- Support Android's share system

### iOS Adaptations

**Human Interface Guidelines**
- Use SF Symbols for icons
- Follow iOS navigation patterns
- Use iOS-specific controls (sheets, popovers)
- Support iOS gesture navigation

**System Integration**
- Support iOS share sheet
- Use iOS notification system
- Follow iOS permission patterns
- Support iOS shortcuts and widgets

---

## Layout Patterns

### Screen Layouts

#### 1. Game Lobby
```
┌─ Navigation Bar ──────────────────┐
│ ← Back    BitCraps        ⚙️     │
├───────────────────────────────────┤
│ Quick Stats Card                  │
├───────────────────────────────────┤
│ Available Games                   │
│ ┌─ Game Card ─┐ ┌─ Game Card ─┐   │
│ │ Host: Alice │ │ Host: Bob   │   │
│ │ Players: 2  │ │ Players: 1  │   │
│ │ Signal: ●●● │ │ Signal: ●●○ │   │
│ └─────────────┘ └─────────────┘   │
├───────────────────────────────────┤
│ [ Create Game ] [ Join Game ]     │
└───────────────────────────────────┘
```

#### 2. Active Game
```
┌─ Game Header ─────────────────────┐
│ Craps Game    Balance: 1,250 chips│
│ Point: 6               Players: 3 │
├───────────────────────────────────┤
│         Dice Display              │
│         🎲    🎲                 │
│         Total: 8                  │
├───────────────────────────────────┤
│ Betting Interface                 │
│ [$10] [$25] [$50] [$100]         │
│ Current Bet: $25                  │
├───────────────────────────────────┤
│ [ 🎲 Roll Dice ]                 │
│ [Pass Line] [Don't Pass] [Leave]  │
└───────────────────────────────────┘
```

### Responsive Breakpoints

**Android**
- Compact: < 600dp width
- Medium: 600-839dp width
- Expanded: ≥ 840dp width

**iOS**
- iPhone SE: 375pt width
- iPhone Standard: 390pt width
- iPhone Plus: 428pt width
- iPad: 744pt+ width

---

## Performance Guidelines

### Rendering Performance

**Frame Rate Targets**
- Sustained 60fps during gameplay
- 30fps minimum in background
- 120fps on ProMotion displays (iOS)

**Memory Usage**
- Android: < 150MB peak usage
- iOS: < 100MB peak usage
- Efficient image loading and caching
- Proper disposal of game assets

### Network Efficiency

**Bluetooth Optimization**
- Minimize scan frequency in background
- Efficient advertising intervals
- Connection pooling for multiple peers
- Graceful handling of connection drops

**Battery Optimization**
- Target: < 5% drain per hour active use
- Adaptive refresh rates based on battery level
- Background task optimization
- CPU-intensive operations on background threads

---

## Testing Strategy

### Visual Testing

**Screenshot Tests**
- All major screens in light/dark mode
- Various device sizes and orientations
- Error states and empty states
- Loading states and animations

**Accessibility Testing**
- Screen reader navigation
- Voice control functionality
- High contrast mode support
- Large text scaling (up to 200%)

### Performance Testing

**Animation Smoothness**
- Frame rate monitoring during gameplay
- Memory leak detection
- CPU usage profiling
- Battery drain measurement

### Cross-Platform Validation

**Feature Parity**
- Identical game mechanics and rules
- Consistent visual appearance
- Similar performance characteristics
- Compatible network protocols

**Platform Integration**
- Native system integration points
- Platform-specific optimizations
- Hardware feature utilization
- Accessibility framework support

---

## Implementation Checklist

### Phase 1: Core Components
- [x] Color system implementation
- [x] Typography scale
- [x] Basic components (cards, buttons, indicators)
- [x] Dice animation system
- [x] Layout grid system

### Phase 2: Advanced Features
- [x] Game state management
- [x] Real-time UI updates
- [x] Cross-platform navigation
- [x] Performance monitoring
- [ ] Battery optimization

### Phase 3: Polish & Optimization
- [ ] Accessibility audit and fixes
- [ ] Performance optimization
- [ ] Platform-specific enhancements
- [ ] Animation refinement
- [ ] User testing feedback integration

---

## Maintenance

### Design Tokens
All design specifications are maintained as design tokens that can be updated centrally and propagated to both platforms:

**Android** (stored in `res/values/tokens.xml`)
**iOS** (stored in Color/Typography assets)

### Version Control
Design system changes follow semantic versioning:
- Major: Breaking changes to component APIs
- Minor: New components or non-breaking enhancements
- Patch: Bug fixes and minor adjustments

### Documentation Updates
This specification is updated with each design system release and includes:
- Change notes and migration guides
- Updated implementation examples
- Performance impact assessments
- Accessibility compliance verification

---

This design system ensures BitCraps delivers a consistent, high-quality gaming experience across all mobile platforms while respecting platform conventions and user expectations.