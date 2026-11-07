/* Shared by both pages. */
(function () {
  /** A number box: click selects it, digits only, Enter or click-away applies, Esc cancels. */
  function numberBox(input, range, onApply) {
    var cancelled = false;
    input.type = 'text';
    input.inputMode = 'numeric';
    input.maxLength = 4;
    input.autocomplete = 'off';
    input.addEventListener('focus', function () { cancelled = false; input.select(); });
    input.addEventListener('input', function () { input.value = input.value.replace(/\D/g, '').slice(0, 4); });
    input.addEventListener('keydown', function (e) {
      if (e.key === 'Enter') { e.preventDefault(); e.stopPropagation(); input.blur(); }
      else if (e.key === 'Escape') { e.preventDefault(); e.stopPropagation(); cancelled = true; input.blur(); }
    });
    input.addEventListener('blur', function () {
      var n = parseInt(input.value, 10);
      if (!cancelled && !isNaN(n)) onApply(Math.min(range[1], Math.max(range[0], n)));
      else if (input.dataset.shown !== undefined) input.value = input.dataset.shown;
    });
    return {
      set: function (v) { input.dataset.shown = String(v); if (document.activeElement !== input) input.value = String(v); },
      focused: function () { return document.activeElement === input; }
    };
  }
  /* Icons drawn like the Mac app's SF Symbols (bell, bell.badge, zzz, cup, bed, hourglass, pause circle).
     All on a 24 x 24 box and centred in it. Filled shapes use the text colour; strokes use currentColor. */
  var S = 'fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round"';
  var BELL = '<path d="M12 2.8a1.6 1.6 0 0 1 1.6 1.6v.45A6.2 6.2 0 0 1 18.2 10.8v3l1.7 2.6a1 1 0 0 1-.85 1.55H4.95a1 1 0 0 1-.85-1.55l1.7-2.6v-3a6.2 6.2 0 0 1 4.6-5.95V4.4A1.6 1.6 0 0 1 12 2.8z"/><path d="M9.6 19.4h4.8a2.4 2.4 0 0 1-4.8 0z"/>';
  var ICONS = {
    bell: BELL,
    bellBadge: '<g transform="translate(-1.5 1) scale(.9)">' + BELL + '</g><circle cx="18.2" cy="5.8" r="3.3"/>',
    zzz: '<g ' + S + ' stroke-width="2.2"><path d="M4 9.5h7L4 18h7"/><path d="M13.5 4h4.8l-4.8 5.8h4.8"/><path d="M19 13h2.6L19 16.2h2.6" stroke-width="1.8"/></g>',
    cup: '<path d="M3.5 6h11.5v4.6a5.75 5.75 0 0 1-11.5 0V6z"/><path ' + S + ' stroke-width="1.9" d="M15 7.6h1.7a2.4 2.4 0 0 1 0 4.8h-2"/><rect x="2" y="17.6" width="16.5" height="1.9" rx=".95"/>',
    bed: '<g fill="none" stroke="currentColor" stroke-width="28" stroke-linecap="round" stroke-linejoin="round"><path d="M64 214V100a38 38 0 0 1 38-38H410a38 38 0 0 1 38 38V214"/><path d="M114 214V175a26 26 0 0 1 26-26H207a26 26 0 0 1 26 26V214"/><path d="M280 214V175a26 26 0 0 1 26-26H373a26 26 0 0 1 26 26V214"/><path d="M39 269V250a36 36 0 0 1 36-36H437a36 36 0 0 1 36 36V269"/><rect x="14" y="269" width="484" height="154" rx="22"/><path d="M46 423V448M466 423V448"/></g>',
    hourglass: '<path d="M6.5 3h11v3.5c0 2.2-1.8 3.7-3.2 5.5 1.4 1.8 3.2 3.3 3.2 5.5V21h-11v-3.5c0-2.2 1.8-3.7 3.2-5.5C8.3 10.2 6.5 8.7 6.5 6.5V3z"/>',
    pause: '<path fill-rule="evenodd" d="M12 2.5a9.5 9.5 0 1 0 0 19 9.5 9.5 0 0 0 0-19zM9.2 8.2h1.9v7.6H9.2zM12.9 8.2h1.9v7.6h-1.9z"/>'
  };
  /** An <svg> string for a named icon. */
  var VIEWBOX = { bed: '-4 40 520 430' };   // the bed is an outline drawn on a 512 grid
  function svg(name, cls) {
    return '<svg class="' + (cls || '') + '" viewBox="' + (VIEWBOX[name] || '0 0 24 24') + '" fill="currentColor" aria-hidden="true">' + ICONS[name] + '</svg>';
  }
  window.BB = { numberBox: numberBox, svg: svg };
})();
