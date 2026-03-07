import { useStore } from "../store";

export default function Toast() {
  const { toast, clearToast } = useStore();

  if (!toast) return null;

  return (
    <div
      className={`fixed bottom-4 right-4 max-w-sm px-4 py-3 rounded-lg shadow-lg text-sm flex items-center gap-3 z-50 ${
        toast.type === "success"
          ? "bg-green-500/90 text-white"
          : "bg-red-500/90 text-white"
      }`}
    >
      <span className="flex-1">{toast.message}</span>
      <button
        onClick={clearToast}
        className="text-white/70 hover:text-white"
      >
        <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
          <path strokeLinecap="round" strokeLinejoin="round" d="M6 18 18 6M6 6l12 12" />
        </svg>
      </button>
    </div>
  );
}
