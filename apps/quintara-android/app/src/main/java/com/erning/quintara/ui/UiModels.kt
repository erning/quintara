package com.erning.quintara.ui

import com.erning.quintara.engine.EngineBoard
import com.erning.quintara.engine.EngineColor
import com.erning.quintara.engine.EngineConfig
import com.erning.quintara.engine.EngineDifficulty
import com.erning.quintara.engine.EnginePoint
import com.erning.quintara.engine.EngineSeat
import com.erning.quintara.engine.EngineSnapshot

enum class Difficulty(val label: String) {
    Easy("Easy"),
    Medium("Medium"),
    Hard("Hard"),
    Master("Master"),
}

enum class RuleSet(val label: String) {
    Freestyle("Freestyle"),
    Standard("Standard"),
    Renju("Renju"),
    Caro("Caro");

    val id: String
        get() = label.lowercase()
}

enum class BoardSize(val value: Int, val label: String) {
    Small(13, "13x13"),
    Classic(15, "15x15"),
    Large(19, "19x19"),
}

enum class StoneColor {
    Black,
    White,
}

enum class OpponentMode(val label: String) {
    Computer("Computer"),
    PassAndPlay("2 Players"),
}

enum class PlayerStone(val label: String) {
    Black("Black"),
    White("White"),
}

data class Stone(
    val row: Int,
    val col: Int,
    val color: StoneColor,
)

data class GameUiState(
    val boardSize: BoardSize = BoardSize.Classic,
    val ruleSet: RuleSet = RuleSet.Freestyle,
    val difficulty: Difficulty = Difficulty.Hard,
    val opponentMode: OpponentMode = OpponentMode.Computer,
    val playerStone: PlayerStone = PlayerStone.Black,
    val botThinkingTimeMs: Long = 5_000,
    val stones: List<Stone> = sampleStones(),
    val lastMove: Pair<Int, Int> = 7 to 9,
) {
    fun toEngineConfig(rapfiAssetDir: String? = null): EngineConfig {
        val engineDifficulty = EngineDifficulty.fromUi(difficulty.label)
        val botSeat = EngineSeat.Bot(
            displayName = engineDifficulty.displayName,
            difficulty = engineDifficulty,
            rapfiAssetDir = rapfiAssetDir.takeIf { engineDifficulty == EngineDifficulty.Master },
        )
        val humanSeat = EngineSeat.Human("You")
        val black = when {
            opponentMode == OpponentMode.PassAndPlay -> EngineSeat.Human("Black")
            playerStone == PlayerStone.Black -> humanSeat
            else -> botSeat
        }
        val white = when {
            opponentMode == OpponentMode.PassAndPlay -> EngineSeat.Human("White")
            playerStone == PlayerStone.White -> humanSeat
            else -> botSeat
        }
        return EngineConfig(
            ruleSetId = ruleSet.id,
            boardSize = boardSize.value,
            black = black,
            white = white,
            botThinkingTimeMs = botThinkingTimeMs,
        )
    }
}

fun EngineSnapshot.toStones(): List<Stone> =
    buildList {
        for (row in 0 until board.height) {
            for (col in 0 until board.width) {
                when (board.colorAt(row, col)) {
                    EngineColor.Black -> add(Stone(row, col, StoneColor.Black))
                    EngineColor.White -> add(Stone(row, col, StoneColor.White))
                    null -> {}
                }
            }
        }
    }

fun EngineSnapshot.lastMovePair(): Pair<Int, Int>? =
    lastMove?.let { it.row to it.col }

fun EngineSnapshot.legalPairs(): Set<Pair<Int, Int>> =
    legalMoves.map { it.row to it.col }.toSet()

fun Pair<Int, Int>.toPoint(): EnginePoint =
    EnginePoint(first, second)

fun EngineBoard.isEmpty(): Boolean =
    cells.all { it == null }

fun sampleStones(): List<Stone> = listOf(
    Stone(7, 7, StoneColor.Black),
    Stone(7, 8, StoneColor.White),
    Stone(8, 7, StoneColor.Black),
    Stone(6, 8, StoneColor.White),
    Stone(8, 8, StoneColor.Black),
    Stone(9, 8, StoneColor.White),
    Stone(6, 7, StoneColor.Black),
    Stone(8, 6, StoneColor.White),
    Stone(7, 9, StoneColor.Black),
)
