import Home from "@/pages/Home";
import ResumeEditor from "@/components/ResumeEditor";
import { useRoute } from "@/lib/route";

export default function Shell() {
  return useRoute() === "editor" ? <ResumeEditor /> : <Home />;
}
