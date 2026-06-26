// Small presentational components extracted from App.tsx (Phase 0 refactor).
import { useEffect, useState } from "react";
import { X } from "lucide-react";
import { ImageAsset, getImageAssetDataUrl } from "../tauri";
import { DisclaimerMode } from "../uiTypes";

export function Stat({ label, value }: { label: string; value: number }) {
  return (
    <div className="stat">
      <span>{label}</span>
      <strong>{Math.round(value)}</strong>
    </div>
  );
}

export function SoulAvatar({ soulName, asset }: { soulName: string; asset?: ImageAsset | null }) {
  return (
    <div className={`avatar ${asset ? "image-avatar" : ""}`} aria-hidden="true">
      {asset ? <AssetImage asset={asset} alt="" /> : soulName.slice(0, 1) || "M"}
    </div>
  );
}

export function AssetImage({ asset, alt }: { asset: ImageAsset; alt: string }) {
  const [src, setSrc] = useState("");
  useEffect(() => {
    let cancelled = false;
    getImageAssetDataUrl(asset.id)
      .then((dataUrl) => {
        if (!cancelled) setSrc(dataUrl);
      })
      .catch(() => {
        if (!cancelled) setSrc("");
      });
    return () => {
      cancelled = true;
    };
  }, [asset.id]);
  return src ? <img src={src} alt={alt} /> : <span className="image-loading">Loading image</span>;
}

export function imageAssetMeta(asset: ImageAsset) {
  const dimensions = asset.width && asset.height ? `${asset.width}x${asset.height}` : "dimensions unknown";
  const mime = asset.mime_type ?? "image";
  return `${mime} / ${dimensions}`;
}

export function ImagePreviewModal({
  asset,
  onClose,
}: {
  asset: ImageAsset | null;
  onClose: () => void;
}) {
  if (!asset) return null;
  return (
    <div className="image-preview-backdrop" role="dialog" aria-modal="true" onClick={onClose}>
      <section className="image-preview-modal" onClick={(event) => event.stopPropagation()}>
        <button type="button" className="image-preview-close" onClick={onClose} aria-label="Close image preview">
          <X size={18} />
        </button>
        <AssetImage asset={asset} alt="Image preview" />
        <div className="image-preview-meta">
          <strong>{asset.source}</strong>
          <span>{imageAssetMeta(asset)}</span>
          {asset.prompt ? <p>{asset.prompt}</p> : null}
        </div>
      </section>
    </div>
  );
}

export function RangeField({
  label,
  value,
  min = 0,
  max = 100,
  onChange,
}: {
  label: string;
  value: number;
  min?: number;
  max?: number;
  onChange: (value: number) => void;
}) {
  return (
    <label className="range-field">
      <span>{label}</span>
      <input
        type="range"
        min={min}
        max={max}
        value={value}
        onChange={(event) => onChange(Number(event.target.value))}
      />
      <strong>{value > 0 && min < 0 ? `+${value}` : value}</strong>
    </label>
  );
}

export function DisclaimerScreen({
  mode,
  understood,
  remember,
  onUnderstoodChange,
  onRememberChange,
  onAccept,
  onClose,
}: {
  mode: DisclaimerMode;
  understood: boolean;
  remember: boolean;
  onUnderstoodChange: (value: boolean) => void;
  onRememberChange: (value: boolean) => void;
  onAccept: () => void;
  onClose: () => void;
}) {
  const isLaunch = mode === "launch";

  return (
    <main className="disclaimer-screen">
      <section className="disclaimer-card" role="dialog" aria-modal={isLaunch} aria-labelledby="disclaimer-title">
        <div className="disclaimer-heading">
          <span className="eyebrow">Before you continue</span>
          <h1 id="disclaimer-title">Mnemosyne Experimental Use Disclaimer</h1>
        </div>

        <div className="disclaimer-copy">
          <p>Mnemosyne is experimental open-source AI roleplay software.</p>
          <p>
            Mnemosyne creates fictional continuity through memory, state tracking, and AI-generated narration. Characters may
            appear persistent, emotionally responsive, or self-aware, but the software does not verify consciousness,
            sentience, or real personhood.
          </p>
          <p>
            You are responsible for how you use this software, what models/providers you connect, what content you generate,
            and how you interpret fictional character continuity.
          </p>
          <p>
            Mnemosyne is not therapy, medical care, legal advice, crisis support, or a substitute for real-world relationships
            or professional help.
          </p>
          <p>
            If you use external API providers, your prompts, character data, and generated responses may be sent to those
            providers according to their own policies. Local use depends on your own configuration.
          </p>
          <p>
            This software may produce emotionally intense, disturbing, intimate, fictional, or misleading outputs. Use personal
            caution, especially during long sessions or emotionally heavy roleplay.
          </p>
          <p>
            By continuing, you acknowledge that Mnemosyne is an experimental fiction and roleplay engine, and that you use it
            at your own discretion.
          </p>
        </div>

        {isLaunch ? (
          <div className="disclaimer-options">
            <label>
              <input
                type="checkbox"
                checked={understood}
                onChange={(event) => onUnderstoodChange(event.target.checked)}
              />
              <span>I understand and want to continue.</span>
            </label>
            <label>
              <input
                type="checkbox"
                checked={remember}
                onChange={(event) => onRememberChange(event.target.checked)}
              />
              <span>Do not show this again</span>
            </label>
          </div>
        ) : null}

        <div className="disclaimer-actions">
          {isLaunch ? (
            <>
              <button type="button" className="ghost-action disclaimer-exit" onClick={() => window.close()}>
                Exit
              </button>
              <button type="button" className="start-chat-button" onClick={onAccept} disabled={!understood}>
                Accept and Continue
              </button>
            </>
          ) : (
            <button type="button" className="start-chat-button" onClick={onClose}>
              Close
            </button>
          )}
        </div>
      </section>
    </main>
  );
}
