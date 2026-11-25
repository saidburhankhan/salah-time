.pragma library

// Use the next 1, 2 or 5 times a power of ten for an open-ended count axis.
function niceMax(value) {
    const magnitude = Math.pow(10, Math.floor(Math.log10(Math.max(1, value))))
    const fraction = value / magnitude
    return magnitude * (fraction <= 1 ? 1 : fraction <= 2 ? 2 : fraction <= 5 ? 5 : 10)
}
