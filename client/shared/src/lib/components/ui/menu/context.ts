import { createContext } from "svelte";

export const [getMenu, setMenu] = createContext<{ close: () => void }>();
