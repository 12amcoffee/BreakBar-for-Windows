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
  window.BB = { numberBox: numberBox };
})();
