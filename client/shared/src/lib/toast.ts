import { toast } from "@zerodevx/svelte-toast";

export const toastOptions = {
  duration: 5000,
  dismissable: true,
};

export function toastError(message: string) {
  toast.push(message, {
    theme: {
      "--toastBackground": "var(--destructive)",
      "--toastColor": "white",
      "--toastBarBackground": "oklch(0.42 0.16 27)",
    },
  });
}

export function toastSuccess(message: string) {
  toast.push(message);
}
