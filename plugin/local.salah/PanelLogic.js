.pragma library

function barStrip(position, width, height, thickness) {
  var side = position === "left" || position === "right"
  var size = Math.min(Math.max(0, thickness), side ? width : height)
  return {
    x: position === "right" ? width - size : 0,
    y: position === "bottom" ? height - size : 0,
    width: side ? size : width,
    height: side ? height : size
  }
}
