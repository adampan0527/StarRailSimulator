import { createRoot } from 'react-dom/client';
import { Diagnostics } from './desktop/Diagnostics';
import './styles.css';

const root = document.getElementById('root');
if (root !== null) {
  createRoot(root).render(<Diagnostics />);
}
