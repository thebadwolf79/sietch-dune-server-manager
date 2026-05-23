import { useState } from "react";

import type { PageId } from "../types/ui";

export function useActivePage(initial: PageId = "servers") {
  const [activePage, setActivePage] = useState<PageId>(initial);
  return { activePage, setActivePage };
}
