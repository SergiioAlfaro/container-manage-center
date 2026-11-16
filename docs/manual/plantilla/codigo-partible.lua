-- SPDX-License-Identifier: GPL-3.0-or-later
-- SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
-- Filtro de pandoc para el manual: el código en línea (rutas, nombres de
-- órdenes, códigos de error) se puede partir después de / . _ - , : para que
-- no se salga del margen ni invada la columna siguiente de una tabla.

local especiales = {
  ["\\"] = "\\textbackslash{}",
  ["{"] = "\\{",
  ["}"] = "\\}",
  ["$"] = "\\$",
  ["&"] = "\\&",
  ["#"] = "\\#",
  ["^"] = "\\textasciicircum{}",
  ["_"] = "\\_",
  ["%"] = "\\%",
  ["~"] = "\\textasciitilde{}",
}

local partibles = { ["/"] = true, ["."] = true, ["_"] = true, ["-"] = true,
  [","] = true, [":"] = true, ["\\"] = true }

function Code(el)
  if not FORMAT:match("latex") then
    return nil
  end
  local partes = {}
  for _, c in utf8.codes(el.text) do
    local ch = utf8.char(c)
    local esc = especiales[ch] or ch
    if ch == " " then
      esc = "\\ "
    end
    table.insert(partes, esc)
    if partibles[ch] then
      table.insert(partes, "\\allowbreak{}")
    end
  end
  return pandoc.RawInline("latex", "\\texttt{" .. table.concat(partes) .. "}")
end
