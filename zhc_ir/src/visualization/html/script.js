// Pan and zoom state
let scale = 1;
let translateX = 0;
let translateY = 0;

// Panning state
let isPanning = false;
let startX = 0;
let startY = 0;

const viewport = document.getElementById('viewport');
const canvas = document.getElementById('canvas');

// Apply current transform
function updateTransform() {
    canvas.style.transform = `translate(${translateX}px, ${translateY}px) scale(${scale})`;
}

// Zoom with mouse wheel (Ctrl/Cmd + scroll)
viewport.addEventListener('wheel', (e) => {
    if (e.ctrlKey || e.metaKey) {
        e.preventDefault();

        const rect = viewport.getBoundingClientRect();
        const mouseX = e.clientX - rect.left;
        const mouseY = e.clientY - rect.top;

        // Zoom factor
        const zoomFactor = e.deltaY > 0 ? 0.9 : 1.1;
        const newScale = Math.min(Math.max(scale * zoomFactor, 0.1), 100);

        // Zoom towards mouse position
        const scaleChange = newScale / scale;
        translateX = mouseX - (mouseX - translateX) * scaleChange;
        translateY = mouseY - (mouseY - translateY) * scaleChange;
        scale = newScale;

        updateTransform();
    }
}, { passive: false });

// Pan with mouse drag
viewport.addEventListener('mousedown', (e) => {
    // Left click only
    if (e.button !== 0) return;

    isPanning = true;
    startX = e.clientX - translateX;
    startY = e.clientY - translateY;
    viewport.classList.add('grabbing');
});

window.addEventListener('mousemove', (e) => {
    if (!isPanning) return;

    translateX = e.clientX - startX;
    translateY = e.clientY - startY;
    updateTransform();
});

window.addEventListener('mouseup', () => {
    isPanning = false;
    viewport.classList.remove('grabbing');
});

// Prevent context menu on right-click (reserve for future use)
viewport.addEventListener('contextmenu', (e) => {
    e.preventDefault();
});

// Reset view on double-click
viewport.addEventListener('dblclick', () => {
    scale = 1;
    translateX = 0;
    translateY = 0;
    updateTransform();
});

// Keyboard shortcuts
window.addEventListener('keydown', (e) => {
    // Reset on 'r' or '0'
    if (e.key === 'r' || e.key === '0') {
        scale = 1;
        translateX = 0;
        translateY = 0;
        updateTransform();
    }
    // Zoom in on '+'
    if (e.key === '+' || e.key === '=') {
        const rect = viewport.getBoundingClientRect();
        const centerX = rect.width / 2;
        const centerY = rect.height / 2;
        const newScale = Math.min(scale * 1.2, 100);
        const scaleChange = newScale / scale;
        translateX = centerX - (centerX - translateX) * scaleChange;
        translateY = centerY - (centerY - translateY) * scaleChange;
        scale = newScale;
        updateTransform();
    }
    // Zoom out on '-'
    if (e.key === '-') {
        const rect = viewport.getBoundingClientRect();
        const centerX = rect.width / 2;
        const centerY = rect.height / 2;
        const newScale = Math.max(scale / 1.2, 0.1);
        const scaleChange = newScale / scale;
        translateX = centerX - (centerX - translateX) * scaleChange;
        translateY = centerY - (centerY - translateY) * scaleChange;
        scale = newScale;
        updateTransform();
    }
});

// Initial transform
updateTransform();

// ============================================
// Hover highlighting for values
// ============================================

const highlightableSelector = '.input-port[data-val], .output-port[data-val], .link[data-val], .link-hitarea[data-val]';

// Pinned values: Map<valId, color>
const pinnedValues = new Map();

function randomColor() {
    // HSL with good saturation and lightness for visibility
    const hue = Math.floor(Math.random() * 360);
    return `hsl(${hue}, 70%, 50%)`;
}

function applyPinnedStyle(el, color) {
    if (el.classList.contains('link') || el.classList.contains('link-hitarea')) {
        el.style.stroke = color;
    } else {
        el.style.outline = `3px solid ${color}`;
    }
}

function clearPinnedStyle(el) {
    el.style.stroke = '';
    el.style.outline = '';
}

function refreshPinnedStyles() {
    // Reapply all pinned styles
    const all = document.querySelectorAll(highlightableSelector);
    all.forEach(el => {
        const valId = el.getAttribute('data-val');
        if (pinnedValues.has(valId)) {
            applyPinnedStyle(el, pinnedValues.get(valId));
            el.classList.add('pinned');
        } else {
            clearPinnedStyle(el);
            el.classList.remove('pinned');
        }
    });
}

function highlightValue(valId) {
    const all = document.querySelectorAll(highlightableSelector);
    all.forEach(el => {
        if (el.getAttribute('data-val') === valId) {
            el.classList.add('highlight');
            el.classList.remove('dimmed');
        } else if (!pinnedValues.has(el.getAttribute('data-val'))) {
            el.classList.add('dimmed');
            el.classList.remove('highlight');
        } else {
            // Pinned but not hovered: don't dim
            el.classList.remove('highlight', 'dimmed');
        }
    });
}

function clearHighlight() {
    const all = document.querySelectorAll(highlightableSelector);
    all.forEach(el => {
        el.classList.remove('highlight', 'dimmed');
    });
}

function togglePin(valId) {
    if (pinnedValues.has(valId)) {
        pinnedValues.delete(valId);
    } else {
        pinnedValues.set(valId, randomColor());
    }
    refreshPinnedStyles();
}

viewport.addEventListener('mouseover', (e) => {
    if (isPanning) return;
    const target = e.target.closest('[data-val]');
    if (target) {
        highlightValue(target.getAttribute('data-val'));
    }
});

viewport.addEventListener('mouseout', (e) => {
    const target = e.target.closest('[data-val]');
    const related = e.relatedTarget ? e.relatedTarget.closest('[data-val]') : null;
    if (target && (!related || related.getAttribute('data-val') !== target.getAttribute('data-val'))) {
        clearHighlight();
    }
});

// Click: data-val click → toggle pin; click on a node body → open color picker.
viewport.addEventListener('click', (e) => {
    const valTarget = e.target.closest('[data-val]');
    if (valTarget) {
        togglePin(valTarget.getAttribute('data-val'));
        return;
    }
    const nodeTarget = e.target.closest('.node');
    if (nodeTarget) {
        openColorPicker(nodeTarget, e.clientX, e.clientY);
    }
});

// ============================================
// Per-node color customization
// ============================================

// Map<nodeId, color> — in-memory, lost on reload (matches pinnedValues).
const nodeColors = new Map();

const PALETTE = [
    '#ff6b6b', '#ffa94d', '#ffd43b', '#69db7c',
    '#4dabf7', '#9775fa', '#f783ac', '#868e96',
];

// The body rect is the V4/V5 background, emitted first by the composition
// primitive — so it's the first direct <rect> child of the node group.
function getNodeBody(group) {
    return group.querySelector(':scope > rect');
}

function applyNodeColor(group, color) {
    const body = getNodeBody(group);
    if (!body) return;
    if (color === null) {
        body.style.fill = '';
        nodeColors.delete(group.id);
    } else {
        body.style.fill = color;
        nodeColors.set(group.id, color);
    }
}

let activePicker = null;

function closeColorPicker() {
    if (activePicker) {
        activePicker.remove();
        activePicker = null;
    }
}

function openColorPicker(group, clientX, clientY) {
    closeColorPicker();
    const picker = document.createElement('div');
    picker.className = 'color-picker';
    picker.style.left = `${clientX}px`;
    picker.style.top = `${clientY}px`;
    for (const color of PALETTE) {
        const swatch = document.createElement('button');
        swatch.className = 'color-swatch';
        swatch.style.background = color;
        swatch.addEventListener('click', (ev) => {
            ev.stopPropagation();
            applyNodeColor(group, color);
            closeColorPicker();
        });
        picker.appendChild(swatch);
    }
    const clear = document.createElement('button');
    clear.className = 'color-swatch color-clear';
    clear.textContent = '×';
    clear.title = 'Reset color';
    clear.addEventListener('click', (ev) => {
        ev.stopPropagation();
        applyNodeColor(group, null);
        closeColorPicker();
    });
    picker.appendChild(clear);
    document.body.appendChild(picker);
    activePicker = picker;
}

// Dismiss on outside mousedown or Escape.
document.addEventListener('mousedown', (e) => {
    if (activePicker && !activePicker.contains(e.target)) {
        closeColorPicker();
    }
}, true);

window.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') closeColorPicker();
});
