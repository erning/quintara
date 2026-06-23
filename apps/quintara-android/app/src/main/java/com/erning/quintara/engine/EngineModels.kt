package com.erning.quintara.engine

import org.json.JSONArray
import org.json.JSONObject

enum class EngineColor(val wire: String) {
    Black("black"),
    White("white");

    fun opposite(): EngineColor = if (this == Black) White else Black

    companion object {
        fun fromWire(value: String): EngineColor =
            entries.firstOrNull { it.wire == value } ?: Black
    }
}

enum class EngineDifficulty(val wire: String, val label: String, val displayName: String) {
    Easy("easy", "Easy", "Sage"),
    Medium("medium", "Medium", "Titan"),
    Hard("hard", "Hard", "Onyx"),
    Master("master", "Master", "Rapfi");

    companion object {
        fun fromUi(label: String): EngineDifficulty =
            entries.firstOrNull { it.label == label } ?: Hard
    }
}

data class EnginePoint(val row: Int, val col: Int)

data class EngineBoard(
    val width: Int,
    val height: Int,
    val cells: List<EngineColor?>,
) {
    fun colorAt(row: Int, col: Int): EngineColor? =
        cells.getOrNull(row * width + col)
}

data class EngineSnapshot(
    val board: EngineBoard,
    val sideToMove: EngineColor,
    val moveHistory: List<EnginePoint>,
    val legalMoves: Set<EnginePoint>,
    val lastMove: EnginePoint?,
    val termination: EngineTermination?,
)

sealed interface EngineWaiting {
    data class Human(val color: EngineColor) : EngineWaiting
    data class Bot(val color: EngineColor) : EngineWaiting
    data object Done : EngineWaiting
}

sealed interface EngineTermination {
    data class Win(val winner: EngineColor) : EngineTermination
    data object Draw : EngineTermination
    data class Forfeit(val winner: EngineColor, val cause: String) : EngineTermination
    data class Aborted(val cause: String, val faultedColor: EngineColor?) : EngineTermination
}

data class EngineStep(
    val waiting: EngineWaiting,
    val snapshot: EngineSnapshot,
)

sealed interface EngineSeat {
    fun toJson(): JSONObject

    data class Human(val displayName: String) : EngineSeat {
        override fun toJson(): JSONObject = JSONObject()
            .put("kind", "human")
            .put("display_name", displayName)
    }

    data class Bot(
        val displayName: String,
        val difficulty: EngineDifficulty,
        val rapfiAssetDir: String? = null,
    ) : EngineSeat {
        override fun toJson(): JSONObject = JSONObject()
            .put("kind", "bot")
            .put("display_name", displayName)
            .put("difficulty", difficulty.wire)
            .also { json ->
                if (rapfiAssetDir != null) {
                    json.put("rapfi_asset_dir", rapfiAssetDir)
                }
            }
    }
}

data class EngineConfig(
    val ruleSetId: String,
    val boardSize: Int,
    val black: EngineSeat,
    val white: EngineSeat,
    val botThinkingTimeMs: Long = 5_000,
) {
    fun toJson(): JSONObject = JSONObject()
        .put("rule_set_id", ruleSetId)
        .put("board_size", boardSize)
        .put("black", black.toJson())
        .put("white", white.toJson())
        .put("bot_thinking_time_ms", botThinkingTimeMs)
}

sealed interface EngineInput {
    fun toJson(): JSONObject

    data class Move(val row: Int, val col: Int) : EngineInput {
        override fun toJson(): JSONObject = JSONObject()
            .put("kind", "move")
            .put("row", row)
            .put("col", col)
    }

    data object Resign : EngineInput {
        override fun toJson(): JSONObject = JSONObject()
            .put("kind", "resign")
    }

    data class Rewind(val toPly: Int) : EngineInput {
        override fun toJson(): JSONObject = JSONObject()
            .put("kind", "rewind")
            .put("to_ply", toPly)
    }
}

object EngineJson {
    fun parseStep(json: String): EngineStep {
        val root = JSONObject(json)
        return EngineStep(
            waiting = parseWaiting(root.getJSONObject("waiting")),
            snapshot = parseSnapshot(root.getJSONObject("snapshot")),
        )
    }

    fun parseSnapshot(json: String): EngineSnapshot =
        parseSnapshot(JSONObject(json))

    private fun parseSnapshot(json: JSONObject): EngineSnapshot {
        val board = parseBoard(json.getJSONObject("board"))
        val legal = json.getJSONArray("legal_moves").points().toSet()
        return EngineSnapshot(
            board = board,
            sideToMove = EngineColor.fromWire(json.getString("side_to_move")),
            moveHistory = json.getJSONArray("move_history").points(),
            legalMoves = legal,
            lastMove = json.optJSONObject("last_move")?.point(),
            termination = json.optJSONObject("termination")?.let(::parseTermination),
        )
    }

    private fun parseBoard(json: JSONObject): EngineBoard {
        val cellsJson = json.getJSONArray("cells")
        val cells = buildList {
            repeat(cellsJson.length()) { index ->
                add(if (cellsJson.isNull(index)) null else EngineColor.fromWire(cellsJson.getString(index)))
            }
        }
        return EngineBoard(
            width = json.getInt("width"),
            height = json.getInt("height"),
            cells = cells,
        )
    }

    private fun parseWaiting(json: JSONObject): EngineWaiting =
        when (json.getString("kind")) {
            "human" -> EngineWaiting.Human(EngineColor.fromWire(json.getString("color")))
            "bot" -> EngineWaiting.Bot(EngineColor.fromWire(json.getString("color")))
            else -> EngineWaiting.Done
        }

    private fun parseTermination(json: JSONObject): EngineTermination =
        when (json.getString("kind")) {
            "win" -> EngineTermination.Win(EngineColor.fromWire(json.getString("winner")))
            "draw" -> EngineTermination.Draw
            "forfeit" -> EngineTermination.Forfeit(
                winner = EngineColor.fromWire(json.getString("winner")),
                cause = json.getString("cause"),
            )

            "aborted" -> EngineTermination.Aborted(
                cause = json.getString("cause"),
                faultedColor = json.optString("faulted_color").takeIf { it.isNotEmpty() }?.let(EngineColor::fromWire),
            )

            else -> EngineTermination.Aborted("unknown", null)
        }

    private fun JSONArray.points(): List<EnginePoint> =
        buildList {
            repeat(length()) { index ->
                add(getJSONObject(index).point())
            }
        }

    private fun JSONObject.point(): EnginePoint =
        EnginePoint(row = getInt("row"), col = getInt("col"))
}
