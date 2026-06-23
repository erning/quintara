package com.erning.quintara.ui

import android.content.Context
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.ElevatedCard
import androidx.compose.material3.FilterChip
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Slider
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.erning.quintara.engine.EngineColor
import com.erning.quintara.engine.EngineDifficulty
import com.erning.quintara.engine.EngineInput
import com.erning.quintara.engine.EnginePoint
import com.erning.quintara.engine.EngineSnapshot
import com.erning.quintara.engine.EngineStep
import com.erning.quintara.engine.EngineTermination
import com.erning.quintara.engine.EngineWaiting
import com.erning.quintara.engine.NativeEngine
import com.erning.quintara.ui.components.BoardCanvas
import kotlinx.coroutines.delay
import java.io.File

private const val RAPFI_ASSET_DIR = "rapfi"

private enum class Route {
    Home,
    NewGame,
    Game,
    Result,
    Review,
    Settings,
}

@Composable
fun QuintaraApp() {
    val context = LocalContext.current
    var route by remember { mutableStateOf(Route.Home) }
    var setup by remember { mutableStateOf(GameUiState()) }
    var session by remember { mutableStateOf<NativeEngine.NativeSession?>(null) }
    var step by remember { mutableStateOf<EngineStep?>(null) }
    var error by remember { mutableStateOf<String?>(null) }
    val rapfiAvailable = remember { NativeEngine.isRapfiAvailable() && hasRapfiAssets(context) }

    DisposableEffect(Unit) {
        onDispose {
            session?.close()
        }
    }

    fun startGame(nextSetup: GameUiState = setup) {
        error = null
        runCatching {
            session?.close()
            val rapfiAssetDir = if (nextSetup.needsRapfi()) ensureRapfiAssets(context) else null
            val created = NativeEngine.createSession(nextSetup.toEngineConfig(rapfiAssetDir))
            val first = created.tick()
            session = created
            step = first
            setup = nextSetup
            route = if (first.snapshot.termination == null) Route.Game else Route.Result
        }.onFailure {
            error = it.message ?: it.toString()
            route = Route.NewGame
        }
    }

    fun tick(input: EngineInput?): EngineStep? {
        val current = session ?: return null
        return runCatching {
            current.tick(input)
        }.onSuccess { next ->
            step = next
            if (next.snapshot.termination != null || next.waiting is EngineWaiting.Done) {
                route = Route.Result
            }
        }.onFailure {
            error = it.message ?: it.toString()
        }.getOrNull()
    }

    Surface(
        modifier = Modifier.fillMaxSize(),
        color = MaterialTheme.colorScheme.surface,
    ) {
        when (route) {
            Route.Home -> HomeScreen(
                setup = setup,
                step = step,
                onNewGame = { route = Route.NewGame },
                onResume = { if (session != null && step != null) route = Route.Game else route = Route.NewGame },
                onSettings = { route = Route.Settings },
            )

            Route.NewGame -> NewGameScreen(
                setup = setup,
                rapfiAvailable = rapfiAvailable,
                error = error,
                onBack = { route = Route.Home },
                onChange = { setup = it },
                onStart = { startGame(it) },
            )

            Route.Game -> {
                val current = step
                if (current == null) {
                    route = Route.Home
                } else {
                    GameScreen(
                        setup = setup,
                        step = current,
                        onBack = { route = Route.Home },
                        onTick = ::tick,
                        onReview = { route = Route.Review },
                    )
                }
            }

            Route.Result -> {
                val current = step
                if (current == null) {
                    route = Route.Home
                } else {
                    ResultScreen(
                        setup = setup,
                        step = current,
                        onReview = { route = Route.Review },
                        onHome = { route = Route.Home },
                        onRematch = { startGame(setup) },
                    )
                }
            }

            Route.Review -> {
                val current = step
                if (current == null) {
                    route = Route.Home
                } else {
                    ReviewScreen(
                        setup = setup,
                        snapshot = current.snapshot,
                        onBack = { route = Route.Game },
                    )
                }
            }

            Route.Settings -> SettingsScreen(
                setup = setup,
                onChange = { setup = it },
                onBack = { route = Route.Home },
            )
        }
    }
}

@Composable
private fun HomeScreen(
    setup: GameUiState,
    step: EngineStep?,
    onNewGame: () -> Unit,
    onResume: () -> Unit,
    onSettings: () -> Unit,
) {
    val previewStones = step?.snapshot?.toStones() ?: setup.stones
    val previewLast = step?.snapshot?.lastMovePair() ?: setup.lastMove
    PhonePage {
        StoneStrip()
        Text(
            text = "quintara",
            fontSize = 40.sp,
            fontWeight = FontWeight.Black,
            color = MaterialTheme.colorScheme.onSurface,
        )
        Text(
            text = "FIVE IN A ROW · 五子棋",
            letterSpacing = 2.sp,
            fontSize = 12.sp,
            color = Color(0xFF8E93A1),
        )
        BoardCard {
            BoardCanvas(
                boardSize = setup.boardSize.value,
                stones = previewStones,
                lastMove = previewLast,
            )
        }
        PrimaryAction("New Game", onNewGame)
        ElevatedCard(
            modifier = Modifier.fillMaxWidth(),
            colors = CardDefaults.elevatedCardColors(containerColor = Color.White),
            onClick = onResume,
        ) {
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(18.dp),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Column {
                    Text("Resume game", fontWeight = FontWeight.Bold)
                    Text(resumeSubtitle(step, setup), color = Color(0xFF7B8090), fontSize = 12.sp)
                }
                Text("›", fontSize = 28.sp, color = Color(0xFF9AA0AE))
            }
        }
        Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            SecondaryAction("Pass & Play", Modifier.weight(1f)) {
                onNewGame()
            }
            SecondaryAction("My Games", Modifier.weight(1f), onResume)
        }
        TextButton(
            modifier = Modifier.align(Alignment.CenterHorizontally),
            onClick = onSettings,
        ) {
            Text("Settings")
        }
    }
}

@Composable
private fun NewGameScreen(
    setup: GameUiState,
    rapfiAvailable: Boolean,
    error: String?,
    onBack: () -> Unit,
    onChange: (GameUiState) -> Unit,
    onStart: (GameUiState) -> Unit,
) {
    PhonePage {
        TopRow("New Game", onBack)
        SectionLabel("OPPONENT")
        Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            ChoiceCard(
                title = "Computer",
                subtitle = "Play the engine",
                selected = setup.opponentMode == OpponentMode.Computer,
                modifier = Modifier.weight(1f),
            ) {
                onChange(setup.copy(opponentMode = OpponentMode.Computer))
            }
            ChoiceCard(
                title = "2 Players",
                subtitle = "Pass & play",
                selected = setup.opponentMode == OpponentMode.PassAndPlay,
                modifier = Modifier.weight(1f),
            ) {
                onChange(setup.copy(opponentMode = OpponentMode.PassAndPlay))
            }
        }
        SectionLabel("DIFFICULTY")
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Difficulty.entries.forEach { difficulty ->
                val enabled = difficulty != Difficulty.Master || rapfiAvailable
                FilterChip(
                    selected = setup.difficulty == difficulty,
                    enabled = enabled && setup.opponentMode == OpponentMode.Computer,
                    onClick = { onChange(setup.copy(difficulty = difficulty)) },
                    label = { Text(difficulty.label) },
                )
            }
        }
        Text(
            text = if (rapfiAvailable) "Master uses Rapfi native library" else "Master unlocks when librapfi.so is packaged",
            color = Color(0xFF8E93A1),
            fontSize = 12.sp,
        )
        SectionLabel("RULE SET")
        WrapChips(
            values = RuleSet.entries,
            selected = setup.ruleSet,
            label = { it.label },
            onSelect = { onChange(setup.copy(ruleSet = it)) },
        )
        SectionLabel("BOARD SIZE")
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            BoardSize.entries.forEach { size ->
                FilterChip(
                    selected = setup.boardSize == size,
                    onClick = { onChange(setup.copy(boardSize = size)) },
                    label = { Text(size.label) },
                )
            }
        }
        SectionLabel("YOUR STONE")
        Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            ChoiceCard("Black", "Moves first", setup.playerStone == PlayerStone.Black, Modifier.weight(1f)) {
                onChange(setup.copy(playerStone = PlayerStone.Black))
            }
            ChoiceCard("White", "Moves second", setup.playerStone == PlayerStone.White, Modifier.weight(1f)) {
                onChange(setup.copy(playerStone = PlayerStone.White))
            }
        }
        if (error != null) {
            Text(error, color = Color(0xFFB84A3A), fontSize = 13.sp)
        }
        Spacer(Modifier.height(24.dp))
        PrimaryAction("Start Game") {
            onStart(setup)
        }
    }
}

@Composable
private fun GameScreen(
    setup: GameUiState,
    step: EngineStep,
    onBack: () -> Unit,
    onTick: (EngineInput?) -> EngineStep?,
    onReview: () -> Unit,
) {
    LaunchedEffect(step.waiting, step.snapshot.moveHistory.size) {
        var current = step
        while (current.waiting is EngineWaiting.Bot && current.snapshot.termination == null) {
            delay(30)
            current = onTick(null) ?: break
        }
    }

    val snapshot = step.snapshot
    val legal = snapshot.legalPairs()
    val humanTurn = step.waiting is EngineWaiting.Human
    Scaffold(
        bottomBar = {
            Surface(color = Color(0xFFF7F8FC)) {
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(16.dp),
                    horizontalArrangement = Arrangement.spacedBy(10.dp),
                ) {
                    SecondaryAction("Undo", Modifier.weight(1f)) {
                        val stepBack = if (setup.opponentMode == OpponentMode.Computer) 2 else 1
                        val toPly = (snapshot.moveHistory.size - stepBack).coerceAtLeast(0)
                        onTick(EngineInput.Rewind(toPly))
                    }
                    SecondaryAction("Review", Modifier.weight(1f), onReview)
                    SecondaryAction("Resign", Modifier.weight(1f)) {
                        if (humanTurn) {
                            onTick(EngineInput.Resign)
                        }
                    }
                }
            }
        },
    ) { padding ->
        PhonePage(
            modifier = Modifier.padding(padding),
        ) {
            TopRow("${setup.ruleSet.label} · ${setup.boardSize.label}", onBack)
            PlayerClock(
                name = playerName(EngineColor.White, setup),
                subtitle = playerSubtitle(EngineColor.White, setup),
                time = "05:00",
                active = snapshot.sideToMove == EngineColor.White,
            )
            BoardCard {
                BoardCanvas(
                    boardSize = snapshot.board.width,
                    stones = snapshot.toStones(),
                    lastMove = snapshot.lastMovePair(),
                    legalMoves = if (humanTurn) legal else emptySet(),
                    onPointSelected = if (humanTurn) {
                        { row, col ->
                            if (row to col in legal) {
                                onTick(EngineInput.Move(row, col))
                            }
                        }
                    } else {
                        null
                    },
                )
            }
            Text(
                text = moveStatus(step),
                modifier = Modifier.align(Alignment.CenterHorizontally),
                color = Color(0xFF8E93A1),
                fontSize = 12.sp,
            )
            PlayerClock(
                name = playerName(EngineColor.Black, setup),
                subtitle = playerSubtitle(EngineColor.Black, setup),
                time = "05:00",
                active = snapshot.sideToMove == EngineColor.Black,
            )
        }
    }
}

@Composable
private fun ResultScreen(
    setup: GameUiState,
    step: EngineStep,
    onReview: () -> Unit,
    onHome: () -> Unit,
    onRematch: () -> Unit,
) {
    val snapshot = step.snapshot
    PhonePage {
        BoardCard {
            BoardCanvas(
                boardSize = snapshot.board.width,
                stones = snapshot.toStones(),
                lastMove = snapshot.lastMovePair(),
            )
        }
        Text(resultTitle(snapshot.termination), fontSize = 30.sp, fontWeight = FontWeight.Black)
        Text(resultSubtitle(snapshot.termination, setup), color = Color(0xFF6DAE8F))
        Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            StatCard(snapshot.moveHistory.size.toString(), "Moves", Modifier.weight(1f))
            StatCard("—", "Time", Modifier.weight(1f))
            StatCard(setup.difficulty.label, EngineDifficulty.fromUi(setup.difficulty.label).displayName, Modifier.weight(1f))
        }
        PrimaryAction("Rematch", onRematch)
        Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            SecondaryAction("Review Game", Modifier.weight(1f), onReview)
            SecondaryAction("Home", Modifier.weight(1f), onHome)
        }
    }
}

@Composable
private fun ReviewScreen(setup: GameUiState, snapshot: EngineSnapshot, onBack: () -> Unit) {
    PhonePage {
        TopRow("Game Review", onBack)
        BoardCard {
            BoardCanvas(
                boardSize = snapshot.board.width,
                stones = snapshot.toStones(),
                lastMove = snapshot.lastMovePair(),
            )
        }
        Text(
            text = "move ${snapshot.moveHistory.size} / ${snapshot.moveHistory.size}",
            modifier = Modifier.align(Alignment.CenterHorizontally),
            fontWeight = FontWeight.Bold,
            color = Color(0xFF6D75E8),
        )
        snapshot.moveHistory.chunked(2).forEachIndexed { index, pair ->
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .background(if (index == snapshot.moveHistory.lastIndex / 2) Color(0xFFEDEFFF) else Color.Transparent)
                    .padding(horizontal = 12.dp, vertical = 8.dp),
                horizontalArrangement = Arrangement.SpaceBetween,
            ) {
                Text("${index + 1}.  ● ${formatPoint(pair[0], setup.boardSize.value)}", fontSize = 13.sp)
                Text(pair.getOrNull(1)?.let { "○ ${formatPoint(it, setup.boardSize.value)}" } ?: "", fontSize = 13.sp)
            }
        }
    }
}

@Composable
private fun SettingsScreen(setup: GameUiState, onChange: (GameUiState) -> Unit, onBack: () -> Unit) {
    var coordinates by remember { mutableStateOf(true) }
    var highlight by remember { mutableStateOf(true) }
    var confirm by remember { mutableStateOf(false) }
    var forbidden by remember { mutableStateOf(true) }
    var sounds by remember { mutableStateOf(true) }
    var thinking by remember { mutableStateOf(setup.botThinkingTimeMs.toFloat() / 1_000f) }

    PhonePage {
        TopRow("Settings", onBack)
        SectionLabel("APPEARANCE")
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            listOf("Dark", "Light", "System").forEachIndexed { index, label ->
                FilterChip(
                    selected = index == 1,
                    onClick = {},
                    label = { Text(label) },
                )
            }
        }
        SectionLabel("GAMEPLAY")
        SettingsToggle("Show coordinates", coordinates) { coordinates = it }
        SettingsToggle("Highlight last move", highlight) { highlight = it }
        SettingsToggle("Confirm each move", confirm) { confirm = it }
        SettingsToggle("Forbidden-move hints", forbidden) { forbidden = it }
        SectionLabel("ENGINE")
        Text("Bot thinking time", fontWeight = FontWeight.Medium)
        Slider(
            value = thinking,
            onValueChange = {
                thinking = it
                onChange(setup.copy(botThinkingTimeMs = (it * 1_000).toLong()))
            },
            valueRange = 1f..10f,
        )
        Text("${thinking.toInt()}.0s", modifier = Modifier.align(Alignment.End), color = Color(0xFFC6973F))
        SectionLabel("SOUND")
        SettingsToggle("Move & win sounds", sounds) { sounds = it }
        Spacer(Modifier.height(24.dp))
        Text(
            text = "quintara · v0.0.1",
            modifier = Modifier.align(Alignment.CenterHorizontally),
            color = Color(0xFF9AA0AE),
            fontSize = 12.sp,
        )
    }
}

@Composable
private fun PhonePage(
    modifier: Modifier = Modifier,
    content: @Composable ColumnScope.() -> Unit,
) {
    Column(
        modifier = modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(horizontal = 22.dp, vertical = 18.dp),
        verticalArrangement = Arrangement.spacedBy(14.dp),
        content = content,
    )
}

@Composable
private fun BoardCard(content: @Composable () -> Unit) {
    ElevatedCard(
        shape = RoundedCornerShape(8.dp),
        colors = CardDefaults.elevatedCardColors(containerColor = Color(0xFFE3BE77)),
        elevation = CardDefaults.elevatedCardElevation(defaultElevation = 10.dp),
        modifier = Modifier.fillMaxWidth(),
    ) {
        Box(Modifier.padding(14.dp)) {
            content()
        }
    }
}

@Composable
private fun PrimaryAction(label: String, onClick: () -> Unit) {
    Button(
        modifier = Modifier
            .fillMaxWidth()
            .height(56.dp),
        colors = ButtonDefaults.buttonColors(containerColor = MaterialTheme.colorScheme.primary),
        shape = RoundedCornerShape(8.dp),
        onClick = onClick,
    ) {
        Text(label, fontWeight = FontWeight.Bold)
    }
}

@Composable
private fun SecondaryAction(label: String, modifier: Modifier, onClick: () -> Unit) {
    OutlinedButton(
        modifier = modifier.height(54.dp),
        shape = RoundedCornerShape(8.dp),
        onClick = onClick,
    ) {
        Text(label, fontWeight = FontWeight.Bold, textAlign = TextAlign.Center)
    }
}

@Composable
private fun TopRow(title: String, onBack: () -> Unit) {
    Row(verticalAlignment = Alignment.CenterVertically) {
        TextButton(onClick = onBack) {
            Text("‹", fontSize = 30.sp)
        }
        Text(title, fontSize = 20.sp, fontWeight = FontWeight.Black)
    }
}

@Composable
private fun SectionLabel(label: String) {
    Text(
        text = label,
        color = Color(0xFF9AA0AE),
        fontSize = 11.sp,
        letterSpacing = 2.sp,
        fontWeight = FontWeight.Bold,
    )
}

@Composable
private fun ChoiceCard(
    title: String,
    subtitle: String,
    selected: Boolean,
    modifier: Modifier,
    onClick: () -> Unit,
) {
    ElevatedCard(
        modifier = modifier.height(88.dp),
        shape = RoundedCornerShape(8.dp),
        colors = CardDefaults.elevatedCardColors(
            containerColor = if (selected) Color(0xFFEDEFFF) else Color.White,
        ),
        onClick = onClick,
    ) {
        Column(
            modifier = Modifier.padding(14.dp),
            verticalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            Text(title, fontWeight = FontWeight.Bold)
            Text(subtitle, color = Color(0xFF7B8090), fontSize = 12.sp)
        }
    }
}

@Composable
private fun PlayerClock(name: String, subtitle: String, time: String, active: Boolean = false) {
    ElevatedCard(
        modifier = Modifier.fillMaxWidth(),
        shape = RoundedCornerShape(8.dp),
        colors = CardDefaults.elevatedCardColors(
            containerColor = if (active) Color(0xFFFFFBF0) else Color.White,
        ),
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(16.dp),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Column {
                Text(name, fontWeight = FontWeight.Bold)
                Text(subtitle, color = Color(0xFF7B8090), fontSize = 12.sp)
            }
            Text(time, fontSize = 22.sp, fontWeight = FontWeight.Black)
        }
    }
}

@Composable
private fun StatCard(value: String, label: String, modifier: Modifier) {
    ElevatedCard(
        modifier = modifier.height(70.dp),
        shape = RoundedCornerShape(8.dp),
        colors = CardDefaults.elevatedCardColors(containerColor = Color.White),
    ) {
        Column(
            modifier = Modifier.fillMaxSize(),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.Center,
        ) {
            Text(value, fontWeight = FontWeight.Black, fontSize = 20.sp)
            Text(label, color = Color(0xFF7B8090), fontSize = 11.sp)
        }
    }
}

@Composable
private fun SettingsToggle(label: String, checked: Boolean, onChange: (Boolean) -> Unit) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(label)
        Switch(checked = checked, onCheckedChange = onChange)
    }
}

@Composable
private fun StoneStrip() {
    Row(horizontalArrangement = Arrangement.spacedBy(7.dp)) {
        listOf(
            StoneColor.Black,
            StoneColor.White,
            StoneColor.Black,
            StoneColor.White,
            StoneColor.Black,
        ).forEach { color ->
            Surface(
                modifier = Modifier.size(18.dp),
                shape = RoundedCornerShape(9.dp),
                color = if (color == StoneColor.Black) Color(0xFF252733) else Color(0xFFF4F0E8),
            ) {}
        }
    }
}

@Composable
private fun <T> WrapChips(
    values: List<T>,
    selected: T,
    label: (T) -> String,
    onSelect: (T) -> Unit,
) {
    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        values.forEach { value ->
            FilterChip(
                selected = value == selected,
                onClick = { onSelect(value) },
                label = { Text(label(value)) },
            )
        }
    }
}

private fun resumeSubtitle(step: EngineStep?, setup: GameUiState): String =
    if (step == null) {
        "No saved game yet"
    } else {
        "${setup.ruleSet.label} · move ${step.snapshot.moveHistory.size}"
    }

private fun playerName(color: EngineColor, setup: GameUiState): String {
    if (setup.opponentMode == OpponentMode.PassAndPlay) {
        return if (color == EngineColor.Black) "Black" else "White"
    }
    val humanColor = if (setup.playerStone == PlayerStone.Black) EngineColor.Black else EngineColor.White
    return if (color == humanColor) "You" else EngineDifficulty.fromUi(setup.difficulty.label).displayName
}

private fun playerSubtitle(color: EngineColor, setup: GameUiState): String {
    val role = if (playerName(color, setup) == "You" || setup.opponentMode == OpponentMode.PassAndPlay) {
        "Human"
    } else {
        "Bot · ${setup.difficulty.label}"
    }
    val stone = if (color == EngineColor.Black) "Black" else "White"
    return "$role · $stone"
}

private fun moveStatus(step: EngineStep): String =
    when (val waiting = step.waiting) {
        is EngineWaiting.Human -> "${waiting.color.wire.replaceFirstChar { it.uppercase() }} to move"
        is EngineWaiting.Bot -> "${waiting.color.wire.replaceFirstChar { it.uppercase() }} thinking"
        EngineWaiting.Done -> "Game finished"
    }

private fun resultTitle(termination: EngineTermination?): String =
    when (termination) {
        is EngineTermination.Win -> "${termination.winner.wire.replaceFirstChar { it.uppercase() }} wins!"
        EngineTermination.Draw -> "Draw"
        is EngineTermination.Forfeit -> "${termination.winner.wire.replaceFirstChar { it.uppercase() }} wins!"
        is EngineTermination.Aborted -> "Game aborted"
        null -> "Game finished"
    }

private fun resultSubtitle(termination: EngineTermination?, setup: GameUiState): String =
    when (termination) {
        is EngineTermination.Win -> "${playerName(termination.winner, setup)} completed five in a row"
        EngineTermination.Draw -> "Board filled without a winner"
        is EngineTermination.Forfeit -> "Forfeit by ${termination.cause}"
        is EngineTermination.Aborted -> "Stopped by ${termination.cause}"
        null -> setup.ruleSet.label
    }

private fun formatPoint(point: EnginePoint, boardSize: Int): String {
    val col = ('A'.code + point.col).toChar()
    val row = boardSize - point.row
    return "$col$row"
}

private fun GameUiState.needsRapfi(): Boolean =
    opponentMode == OpponentMode.Computer && difficulty == Difficulty.Master

private fun hasRapfiAssets(context: Context): Boolean =
    runCatching {
        val names = context.assets.list(RAPFI_ASSET_DIR).orEmpty()
        "config.toml" in names && names.any { it.endsWith(".bin") || it.endsWith(".bin.lz4") }
    }.getOrDefault(false)

private fun ensureRapfiAssets(context: Context): String {
    val targetDir = File(context.filesDir, RAPFI_ASSET_DIR)
    check(targetDir.exists() || targetDir.mkdirs()) {
        "Cannot create Rapfi asset directory: ${targetDir.absolutePath}"
    }

    val names = context.assets.list(RAPFI_ASSET_DIR).orEmpty()
    check(names.isNotEmpty()) { "Rapfi assets are not packaged" }
    names.forEach { name ->
        copyRapfiAsset(context, name, File(targetDir, name))
    }
    check(File(targetDir, "config.toml").isFile) { "Rapfi config.toml is not packaged" }
    return targetDir.absolutePath
}

private fun copyRapfiAsset(context: Context, name: String, target: File) {
    val assetPath = "$RAPFI_ASSET_DIR/$name"
    context.assets.open(assetPath).use { input ->
        val assetSize = input.available().toLong()
        if (target.isFile && target.length() == assetSize) {
            return
        }
        target.outputStream().use { output ->
            input.copyTo(output)
        }
    }
}
