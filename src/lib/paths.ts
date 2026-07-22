/** Remote-Pfade sind IMMER Unix-Pfade mit "/" — nie window-seitige Path-APIs benutzen. */
export function joinRemote(dir: string, name: string): string {
  const base = dir.replace(/\/+$/, "");
  return base === "" ? `/${name}` : `${base}/${name}`;
}

export function parentRemote(p: string): string {
  const t = p.replace(/\/+$/, "");
  const i = t.lastIndexOf("/");
  return i <= 0 ? "/" : t.slice(0, i);
}

export function basenameRemote(p: string): string {
  const t = p.replace(/\/+$/, "");
  if (t === "") return "/";
  return t.slice(t.lastIndexOf("/") + 1);
}
