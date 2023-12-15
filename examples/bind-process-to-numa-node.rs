use hwloc2::{Topology, CpuBindFlags, TopologyObject, ObjectType};

fn main() {
    let mut topo = Topology::new().unwrap();
    let packages = topo.objects_with_type(&ObjectType::Package).unwrap();
    for package in packages.iter() {
        println!("{} #{}", package, package.os_index());
    }

    let cpuset = packages[0].cpuset().unwrap();
    println!("cpuset of packages[0] = {}", cpuset);

    let before = topo.get_cpubind(CpuBindFlags::CPUBIND_PROCESS).unwrap();
    topo.set_cpubind(cpuset, CpuBindFlags::CPUBIND_PROCESS).unwrap();
    let after = topo.get_cpubind(CpuBindFlags::CPUBIND_PROCESS).unwrap();
    println!("before = {before}");
    println!("after = {after}");
}


