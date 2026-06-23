package com.erning.quintara.ui.components

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.dp
import com.erning.quintara.ui.Stone
import com.erning.quintara.ui.StoneColor

@Composable
fun BoardCanvas(
    boardSize: Int,
    stones: List<Stone>,
    lastMove: Pair<Int, Int>?,
    legalMoves: Set<Pair<Int, Int>> = emptySet(),
    onPointSelected: ((row: Int, col: Int) -> Unit)? = null,
    modifier: Modifier = Modifier,
) {
    var canvasSize by remember { mutableStateOf(IntSize.Zero) }
    Canvas(
        modifier = modifier
            .fillMaxWidth()
            .aspectRatio(1f)
            .onSizeChanged { canvasSize = it }
            .pointerInput(boardSize, canvasSize, onPointSelected) {
                if (onPointSelected == null) {
                    return@pointerInput
                }
                detectTapGestures { offset ->
                    val hit = hitPoint(
                        boardSize = boardSize,
                        width = canvasSize.width.toFloat(),
                        height = canvasSize.height.toFloat(),
                        offset = offset,
                    )
                    if (hit != null) {
                        onPointSelected(hit.first, hit.second)
                    }
                }
            },
    ) {
        val boardPadding = size.minDimension * 0.08f
        val gridSize = size.minDimension - boardPadding * 2f
        val cell = gridSize / (boardSize - 1)
        val origin = Offset(boardPadding, boardPadding)
        val boardCorner = 18.dp.toPx()

        drawRoundRect(
            brush = Brush.linearGradient(
                colors = listOf(Color(0xFFE8C783), Color(0xFFCFA965)),
                start = Offset.Zero,
                end = Offset(size.width, size.height),
            ),
            cornerRadius = androidx.compose.ui.geometry.CornerRadius(boardCorner, boardCorner),
        )

        repeat(boardSize) { index ->
            val p = boardPadding + cell * index
            drawLine(
                color = Color(0xFF9B7B45),
                start = Offset(origin.x, p),
                end = Offset(origin.x + gridSize, p),
                strokeWidth = 1.dp.toPx(),
                cap = StrokeCap.Square,
            )
            drawLine(
                color = Color(0xFF9B7B45),
                start = Offset(p, origin.y),
                end = Offset(p, origin.y + gridSize),
                strokeWidth = 1.dp.toPx(),
                cap = StrokeCap.Square,
            )
        }

        starPoints(boardSize).forEach { (row, col) ->
            drawCircle(
                color = Color(0xFF7D6335),
                radius = 2.2.dp.toPx(),
                center = point(origin, cell, row, col),
            )
        }

        legalMoves.forEach { (row, col) ->
            drawCircle(
                color = Color(0x33747CF4),
                radius = cell * 0.18f,
                center = point(origin, cell, row, col),
            )
        }

        stones.forEach { stone ->
            val center = point(origin, cell, stone.row, stone.col)
            val radius = cell * 0.38f
            val colors = when (stone.color) {
                StoneColor.Black -> listOf(Color(0xFF474A58), Color(0xFF12131B))
                StoneColor.White -> listOf(Color(0xFFFFFFFF), Color(0xFFE8E3D8))
            }
            drawCircle(
                brush = Brush.radialGradient(colors, center, radius * 1.4f),
                radius = radius,
                center = center,
            )
        }

        if (lastMove != null) {
            val center = point(origin, cell, lastMove.first, lastMove.second)
            drawCircle(
                color = Color(0xFFF4C45F),
                radius = cell * 0.48f,
                center = center,
                style = Stroke(width = 2.dp.toPx()),
            )
        }
    }
}

private fun point(origin: Offset, cell: Float, row: Int, col: Int): Offset =
    Offset(origin.x + cell * col, origin.y + cell * row)

private fun hitPoint(boardSize: Int, width: Float, height: Float, offset: Offset): Pair<Int, Int>? {
    if (boardSize < 2 || width <= 0f || height <= 0f) {
        return null
    }
    val side = minOf(width, height)
    val padding = side * 0.08f
    val grid = side - padding * 2f
    val cell = grid / (boardSize - 1)
    val col = ((offset.x - padding + cell / 2f) / cell).toInt()
    val row = ((offset.y - padding + cell / 2f) / cell).toInt()
    return if (row in 0 until boardSize && col in 0 until boardSize) row to col else null
}

private fun starPoints(boardSize: Int): List<Pair<Int, Int>> {
    if (boardSize < 13) {
        return emptyList()
    }
    val low = 3
    val high = boardSize - 4
    val mid = boardSize / 2
    return listOf(
        low to low,
        low to mid,
        low to high,
        mid to low,
        mid to mid,
        mid to high,
        high to low,
        high to mid,
        high to high,
    )
}
