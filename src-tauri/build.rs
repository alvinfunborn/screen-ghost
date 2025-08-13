use std::fs;
use std::path::Path;

fn main() {
    // 复制Python文件到资源目录
    copy_python_files();
    
    tauri_build::build()
}

fn copy_python_files() {
    let python_src = Path::new("python");
    let python_dst = Path::new("src-tauri/python");
    
    if python_src.exists() {
        if let Err(e) = fs::create_dir_all(python_dst) {
            eprintln!("Failed to create python directory: {}", e);
            return;
        }
        
        if let Err(e) = copy_dir_all(python_src, python_dst) {
            eprintln!("Failed to copy python files: {}", e);
        } else {
            println!("Python files copied successfully");
        }
    }
}

fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    if !dst.exists() {
        fs::create_dir(dst)?;
    }
    
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        
        if ty.is_dir() {
            copy_dir_all(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }
    
    Ok(())
}
