// Minimal types for qrcode-terminal (no @types package, to avoid a dependency).
declare module "qrcode-terminal" {
  interface QrcodeTerminal {
    generate(input: string, opts?: { small?: boolean }, cb?: (qr: string) => void): void;
    setErrorLevel(level: "L" | "M" | "Q" | "H"): void;
  }
  const qrcode: QrcodeTerminal;
  export default qrcode;
}
