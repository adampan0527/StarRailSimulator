import { createRoot } from 'react-dom/client';
import { Workbench } from './training/Workbench';
import './styles.css';
import './team/team.css';
import './characters/characters.css';

const root = document.getElementById('root');
if (root !== null) {
  createRoot(root).render(<Workbench />);
}
