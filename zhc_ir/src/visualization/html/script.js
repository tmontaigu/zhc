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

// Double-click routing: value → toggle pin; node body → color picker;
// empty background → reset view.
viewport.addEventListener('dblclick', (e) => {
    const valTarget = e.target.closest('[data-val]');
    if (valTarget) {
        togglePin(valTarget.getAttribute('data-val'));
        return;
    }
    const nodeTarget = e.target.closest('.node');
    if (nodeTarget) {
        openColorPicker(nodeTarget, e.clientX, e.clientY);
        return;
    }
    scale = 1;
    translateX = 0;
    translateY = 0;
    updateTransform();
});

// Keyboard shortcuts
window.addEventListener('keydown', (e) => {
    // Ignore navigation shortcuts while typing in a text field.
    if (e.target instanceof HTMLInputElement) return;
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

// ============================================
// In-canvas search (highlight + center)
// ============================================

const SVG_NS = 'http://www.w3.org/2000/svg';

const searchBox = document.getElementById('search-box');
const searchInput = document.getElementById('search-input');
const searchCount = document.getElementById('search-count');

// Current result set and which one is focused.
let searchMatches = [];   // array of <text> elements
let searchIndex = -1;
let searchBoxes = [];      // highlight <rect>s we injected, for cleanup

function clearSearchBoxes() {
    searchBoxes.forEach(r => r.remove());
    searchBoxes = [];
}

// Draw a highlight rect behind `textEl`. getBBox() is in the text's own user
// space, which is the parent group's space — so a sibling rect with those
// coords lines up regardless of ancestor transforms.
function addSearchBox(textEl, isCurrent) {
    const bbox = textEl.getBBox();
    const pad = bbox.height * 0.15;
    const rect = document.createElementNS(SVG_NS, 'rect');
    rect.setAttribute('x', bbox.x - pad);
    rect.setAttribute('y', bbox.y - pad);
    rect.setAttribute('width', bbox.width + 2 * pad);
    rect.setAttribute('height', bbox.height + 2 * pad);
    rect.setAttribute('rx', pad);
    rect.setAttribute('class', isCurrent ? 'search-hit search-hit-current' : 'search-hit');
    textEl.parentNode.insertBefore(rect, textEl);
    searchBoxes.push(rect);
}

function redrawSearchBoxes() {
    clearSearchBoxes();
    searchMatches.forEach((el, i) => addSearchBox(el, i === searchIndex));
}

// Pan/zoom so `el` sits centered in the viewport at a readable size. We invert
// the current transform to recover the element's canvas-local center, then
// solve for the translate that maps it to the viewport center at targetScale.
function centerOnElement(el) {
    const vp = viewport.getBoundingClientRect();
    const rect = el.getBoundingClientRect();

    const screenX = rect.left + rect.width / 2 - vp.left;
    const screenY = rect.top + rect.height / 2 - vp.top;
    const localX = (screenX - translateX) / scale;
    const localY = (screenY - translateY) / scale;

    // Pick a zoom that renders the match at a comfortable on-screen height.
    const localHeight = rect.height / scale;
    const targetScale = Math.min(Math.max(48 / localHeight, 1), 8);

    scale = targetScale;
    translateX = vp.width / 2 - targetScale * localX;
    translateY = vp.height / 2 - targetScale * localY;
    updateTransform();
}

function updateSearchCount() {
    if (!searchInput.value) {
        searchCount.textContent = '';
        searchCount.classList.remove('no-match');
    } else if (searchMatches.length === 0) {
        searchCount.textContent = '0/0';
        searchCount.classList.add('no-match');
    } else {
        searchCount.textContent = `${searchIndex + 1}/${searchMatches.length}`;
        searchCount.classList.remove('no-match');
    }
}

function runSearch(term) {
    const needle = term.toLowerCase();
    searchMatches = needle
        ? Array.from(document.querySelectorAll('#canvas text'))
            .filter(t => t.textContent.toLowerCase().includes(needle))
        : [];
    searchIndex = searchMatches.length ? 0 : -1;
    redrawSearchBoxes();
    updateSearchCount();
    if (searchIndex >= 0) centerOnElement(searchMatches[searchIndex]);
}

function stepSearch(delta) {
    if (!searchMatches.length) return;
    searchIndex = (searchIndex + delta + searchMatches.length) % searchMatches.length;
    redrawSearchBoxes();
    updateSearchCount();
    centerOnElement(searchMatches[searchIndex]);
}

function openSearch() {
    searchBox.classList.remove('hidden');
    searchInput.focus();
    searchInput.select();
}

function closeSearch() {
    searchBox.classList.add('hidden');
    clearSearchBoxes();
    searchMatches = [];
    searchIndex = -1;
}

// Ctrl/Cmd+F opens our search instead of the browser's.
window.addEventListener('keydown', (e) => {
    if ((e.ctrlKey || e.metaKey) && e.key === 'f') {
        e.preventDefault();
        openSearch();
    }
});

searchInput.addEventListener('input', () => runSearch(searchInput.value));

searchInput.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') {
        e.preventDefault();
        stepSearch(e.shiftKey ? -1 : 1);
    } else if (e.key === 'Escape') {
        e.preventDefault();
        closeSearch();
    }
});

document.getElementById('search-next').addEventListener('click', () => stepSearch(1));
document.getElementById('search-prev').addEventListener('click', () => stepSearch(-1));
document.getElementById('search-close').addEventListener('click', closeSearch);
